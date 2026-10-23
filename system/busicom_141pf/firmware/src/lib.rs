//! Busicom 141-PF calculator firmware (1971), reimplemented in Rust.
//!
//! `no_std`, no allocation, no integer wider than `u8`. This is the
//! machine-independent core: keys in, printed rows and lamps out. The 4004
//! drivers (keyboard scan, printer drum, lamps) sit behind `hal` (milestone M3).
//!
//! Behaviour is specified in `spec/spec.md`; the oracle is the recovered ROM's
//! output for the 42 manual examples (`oracle/manual_examples.json`). The state
//! machine follows the annotated disassembly's operation table (§4.4): working
//! register `WR`, dividend/multiplicand `DR`, constant `CR`, result `RR`,
//! subtotal `SR`, main total `TR`, memory `MR`, with the `MODE`/`MOP`/`MENT`
//! states that decide what `=`, `*`, `/`, `%` and the memory keys do.
//!
//! Since issue #18 (Layer B) every number and flag lives in 4002 RAM, as in
//! the original: the registers are [`bcd::Num`] handles, the flags are status
//! characters, and computed values go through result registers instead of
//! being copied around. On the host the RAM is a model (see [`ram`]); the
//! target reaches the real 4002s.
#![cfg_attr(target_os = "none", no_std)]

pub mod bcd;
pub mod driver_keyboard;
pub mod driver_lamps;
pub mod driver_printer;
pub mod hal;
pub mod keys;
pub mod print;
pub mod ram;

pub use bcd::Num;
pub use keys::{Key, Lamps, Rounding, Switches};
pub use print::{Printer, Row, SymA, SymB};

use bcd::{CAPACITY, CR, DR, FLOAT, MR, SR, T1, T2, TR, WR};
use ram::{srd, swr};

// --- flags: status character S3 of registers 0..7 --------------------------
/// Digits are being typed into WR (or CE set it to an explicit zero).
const ENTERING: u8 = 0x03;
/// `.` was typed in the current entry.
const POINT: u8 = 0x13;
/// The pending mul/div operand (DR) came from a freshly typed entry. This
/// controls the scale of a later repeated `=`/`M=±` operand row: entry-fed
/// operands print at their entered scale, repeat-fed ones at the DP.
const OP_FROM_ENTRY: u8 = 0x23;
/// `MODE` (disassembly `RR.S2`): what the last arithmetic step was.
const MODE: u8 = 0x33;
/// `MOP` (disassembly `DR.S2`): the pending multiply/divide operation.
const MOP: u8 = 0x43;
const LAMP_MEMORY: u8 = 0x53;
const LAMP_OVERFLOW: u8 = 0x63;
// (The minus lamp is WR's sign, read when the lamps are written: no flag.)

// MODE values.
/// A fresh number is available (new entry, or a finished result).
const NEW: u8 = 0;
/// The last operation was multiply or divide.
const MUL_DIV: u8 = 1;
/// The last operation was addition or subtraction.
const ADD_SUB: u8 = 2;

// MOP values.
/// No multiply/divide started (default behaviour is divide-by-DR).
const OP_NONE: u8 = 0;
/// DR is multiplied by the next operand.
const OP_MUL: u8 = 1;
/// DR is divided by the next operand.
const OP_DIV: u8 = 2;
/// `%` codes for [`Calculator::do_mul_div_equals`] (0: `=`, `M=±`): a
/// floating quotient, and for a percent of a division the result x100.
const PCT: u8 = 1;
const PCT_X100: u8 = 2;
// `dispatch` maps keyboard columns 1/2 to `3 - col`.
const _: () = assert!(OP_MUL == 1 && OP_DIV == 2);
/// Constant divide: the divisor is held in CR.
const OP_CONST_DIV: u8 = 3;
/// Constant multiply: the multiplicand is held in CR.
const OP_CONST_MUL: u8 = 4;

/// The selectors in force for the key being processed (latched by `press`
/// into the key-FIFO register's free status characters).
pub const SW_DP: u8 = 0xC2;
pub const SW_ROUND: u8 = 0xC3;

/// Decimal selector (0..8).
#[inline(always)]
fn dp() -> u8 {
    srd(SW_DP)
}
/// Rounding selector (`Rounding as u8`).
#[inline(always)]
fn rounding() -> u8 {
    srd(SW_ROUND)
}

#[inline(always)]
fn get(f: u8) -> u8 {
    srd(f)
}
#[inline(always)]
fn is(f: u8) -> bool {
    srd(f) != 0
}
#[inline(always)]
fn set(f: u8, v: u8) {
    swr(f, v)
}

/// The calculator. All of its state is in 4002 RAM (registers + status
/// characters); the value only names it.
#[derive(Debug)]
pub struct Calculator {
    _ram: (),
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Calculator {
    /// Power-on state: all working registers and memory zero, flags clear.
    pub fn new() -> Self {
        reset(0x80);
        Calculator { _ram: () }
    }

    pub fn lamps(&self) -> Lamps {
        Lamps {
            memory: is(LAMP_MEMORY),
            overflow: is(LAMP_OVERFLOW),
            negative: WR.is_neg(),
        }
    }

    /// Process one key with the selector positions read at that moment.
    ///
    /// After the key is handled, the front-panel **minus-sign lamp** is set from
    /// the working register's sign, exactly as the original firmware's
    /// status-light routine does: the annotated listing (status-light
    /// handling, `010 jms $173 ... 01d wmp`) builds the RAM1 nibble as
    /// `8*WR.S0.bit1 + 4*WR.S0.bit0 + 2*overflow + (MR!=0)`, i.e. the minus lamp
    /// (bit 2) is `WR.S0.bit0`: the sign of the number currently in WR, read at
    /// idle, NOT the accumulator's running sign. See `driver_lamps` for bit 3.
    #[inline(always)]
    pub fn press<P: Printer>(&mut self, key: Key, sw: Switches, out: &mut P) {
        // The selectors read with this key, latched where every step reads them.
        set(SW_DP, sw.dp);
        set(SW_ROUND, sw.rounding as u8);
        self.press_latched(key, out);
    }

    /// [`Calculator::press`] with the selectors already latched in RAM by
    /// [`driver_keyboard::KeyboardDriver::scan_latch`] (the target's loop).
    #[inline(always)]
    pub fn press_latched<P: Printer>(&mut self, key: Key, out: &mut P) {
        self.dispatch(key, out);
    }

    /// The RAM1 lamp nibble ([`driver_lamps::lamp_nibble`] of
    /// [`Calculator::lamps`]), built from the flags without the `Lamps` value.
    #[inline(always)]
    pub fn lamp_nibble(&self) -> u8 {
        use hal::{RAM1_MEMORY, RAM1_NEGATIVE, RAM1_OVERFLOW, RAM1_STATUS};
        // Only constant ORs (a variable | variable is a bit-serial libcall on
        // the 4004).
        let mut n = if is(LAMP_MEMORY) { RAM1_MEMORY } else { 0 };
        if is(LAMP_OVERFLOW) {
            n |= RAM1_OVERFLOW | RAM1_STATUS;
            if WR.is_neg() {
                n |= RAM1_NEGATIVE;
            }
        } else if WR.is_neg() {
            n |= RAM1_NEGATIVE | RAM1_STATUS;
        }
        n
    }

    /// [`Calculator::press_latched`] for the key at keyboard `row` (0-7) and
    /// `col` (0-3), as the target's scanner reports it (row 3 columns 2-3 carry
    /// no key and are ignored).
    #[inline(always)]
    pub fn press_at<P: Printer>(&mut self, row: u8, col: u8, out: &mut P) {
        self.dispatch_at(row, col, out);
    }

    #[inline(always)]
    fn dispatch<P: Printer>(&mut self, key: Key, out: &mut P) {
        let k = key as u8 - Key::Cm as u8;
        self.dispatch_at(k >> 2, k & 3, out);
    }

    /// Dispatch on the key's matrix position (platform/io_map.md): row and
    /// column are nibbles, so every test is a one-nibble compare.
    #[inline(always)]
    fn dispatch_at<P: Printer>(&mut self, row: u8, col: u8, out: &mut P) {
        // At overflow, only CE and C (row 7, columns 2-3) respond
        // (disassembly: the engine starts only for those two keys when SR.S2
        // overflow is set).
        if is(LAMP_OVERFLOW) && (row != 7 || col < 2) {
            return;
        }
        if row >= 4 {
            if row == 7 {
                // SIGN, EX, CE, C
                if col == 0 {
                    self.begin_entry();
                    WR.set_neg(!WR.is_neg());
                } else if col == 1 {
                    self.exchange(out);
                } else if col == 2 {
                    WR.clear();
                    set(ENTERING, 1);
                    set(POINT, 0);
                    set(LAMP_OVERFLOW, 0);
                    set(MODE, NEW);
                } else {
                    self.clear(out);
                }
            } else if col < 3 {
                // rows 4-6: 9 6 3 / 8 5 2 / 7 4 1
                self.digit(13 - row - col - col - col);
            } else if row == 4 {
                self.begin_entry();
                set(POINT, 1);
            } else {
                // 0 (row 6), 00 (row 5)
                self.digit(0);
                if row == 5 {
                    self.digit(0);
                }
            }
            return;
        }
        if row == 0 {
            // CM, RM, M-, M+
            if col == 0 {
                self.clear_memory(out);
            } else if col == 1 {
                self.recall_memory(out);
            } else {
                self.memory_add(col == 2, out);
            }
        } else if row == 1 {
            // SQRT, %, M=-, M=+
            if col == 0 {
                self.sqrt(out);
            } else if col == 1 {
                // `=` and `%` end in the shared mul/div step, called from here
                // so the target call graph stays within the 4004's 3-level
                // stack.
                let pct = self.percent(out);
                self.do_mul_div_equals(out, SymB::Total, pct);
            } else {
                // M=-, M=+: the second operand, then the shared step adds the
                // result into memory.
                self.eq_operand(out);
                let sym_b = if col == 2 {
                    SymB::MemMinus
                } else {
                    SymB::MemPlus
                };
                self.do_mul_div_equals(out, sym_b, 0);
            }
        } else if row == 2 {
            // ST, /, *, =
            if col == 0 {
                self.subtotal_key(out);
            } else if col == 3 {
                if self.equals(out) {
                    self.eq_operand(out);
                    self.do_mul_div_equals(out, SymB::Total, 0);
                }
            } else {
                // column 1 = OP_DIV (2), column 2 = OP_MUL (1)
                self.mul_div_key(3 - col, out);
            }
        } else if col < 2 {
            // -, + (row 3; columns 2-3 carry no key)
            self.add_sub(col == 0, out);
        }
    }

    // --- number entry ---------------------------------------------------

    #[inline(never)]
    fn begin_entry(&mut self) {
        // A function key leaves MODE != New; the next digit starts a new entry.
        if !is(ENTERING) {
            WR.clear();
            set(POINT, 0);
            set(ENTERING, 1);
            set(MODE, NEW);
        }
    }

    #[inline(never)]
    fn digit(&mut self, digit: u8) {
        self.begin_entry();
        let (d, s) = (WR.digits(), WR.scale());
        let typed = if d > s { d } else { s };
        if typed >= CAPACITY {
            return;
        }
        WR.shl10(1);
        WR.set_digit(0, digit);
        if is(POINT) {
            WR.set_scale(s + 1);
        }
    }

    /// Latch a typed entry as the operand for a function key; returns whether
    /// there was one (a fresh operand).
    #[inline(never)]
    fn take_entry(&mut self) -> bool {
        let fresh = is(ENTERING);
        if fresh {
            set(ENTERING, 0);
            set(POINT, 0);
        }
        fresh
    }

    // --- clear ----------------------------------------------------------

    #[inline(never)]
    fn clear<P: Printer>(&mut self, out: &mut P) {
        // C clears WR, DR, CR, SR, TR (and the scratch T1, T2) with every flag
        // but the memory lamp: ENTERING, POINT, OP_FROM_ENTRY, MODE = NEW, MOP =
        // OP_NONE and the overflow and negative lamps are the S3 flags of
        // registers 0-4, 6, 7. Memory (register 5) is preserved.
        reset(MR.0);
        // The original ROM prints the cleared value `0` with the C symbol, not
        // a blank row (verified against oracle/rom_port_trace.txt; see
        // failure_notes/2026-10-18-busicom_141pf-clear-prints-zero.md).
        out.number(WR, SymA::Clear, SymB::None);
    }

    #[inline(always)]
    fn overflow<P: Printer>(&mut self, out: &mut P) {
        set(LAMP_OVERFLOW, 1);
        out.overflow();
    }

    // --- addition / subtraction ----------------------------------------

    #[inline(never)]
    fn add_sub<P: Printer>(&mut self, minus: bool, out: &mut P) {
        // Operand: the new entry, else repeat the last working value.
        self.take_entry();
        // ASSUMPTION (spec A1): extra fractional digits are truncated on align.
        if !operand_shown(false) || T1.width() > CAPACITY {
            self.overflow(out);
            return;
        }
        WR.copy_from(T1);
        let sym = if minus { SymA::Minus } else { SymA::Plus };
        out.number(WR, sym, SymB::None);
        // Both accumulators must fit before either is committed (WR holds the
        // operand, so T1 and T2 take the two candidate sums).
        if Num::add_signed(T2, SR, WR, minus) && Num::add_signed(T1, TR, WR, minus) {
            SR.copy_from(T2);
            TR.copy_from(T1);
            set(MODE, ADD_SUB);
        } else {
            self.overflow(out);
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
    fn mul_div_key<P: Printer>(&mut self, op: u8, out: &mut P) {
        let entered = self.take_entry();
        let mode = get(MODE);
        let mop = get(MOP);
        let sym = sym_for(op);
        if mode == MUL_DIV && !entered {
            // Operation correction: the operand stays latched; the ROM reprints
            // only the op glyph with a blank value (spec A6).
            out.blank(sym, SymB::None);
            set(MOP, op);
            return;
        }
        // Chain: a new op after a pending multiply/divide computes it, and the
        // product/quotient feeds the next op.
        let chain = mode == NEW && (mop == OP_MUL || mop == OP_DIV);
        // A fresh entry echoes at its entered scale; a recalled, accumulated
        // or computed operand is aligned to DP (10-1 `10.00 ×` after `10 M+`;
        // 9-1 `11 ×` keeps its `11`).
        let mut fresh = entered;
        if mode == ADD_SUB {
            // Mixed calc: the main total becomes the first operand and the
            // accumulators clear.
            WR.copy_from(TR);
            SR.clear();
            TR.clear();
            fresh = false;
        }
        operand_shown(fresh);
        out.number(T1, sym, SymB::None);
        if chain {
            if !self.compute() {
                self.overflow(out);
                return;
            }
            WR.copy_from(T2);
            fresh = false;
        }
        DR.copy_from(WR);
        CR.copy_from(WR);
        set(MOP, op);
        set(MODE, MUL_DIV);
        set(OP_FROM_ENTRY, fresh as u8);
    }

    /// Evaluate `DR (op) WR` (or the constant variant) into T2: `__n_compute`
    /// in asm/num.s on the target, [`compute_pending`] on the host.
    #[inline(always)]
    fn compute(&mut self) -> bool {
        #[cfg(target_os = "none")]
        #[allow(unused_unsafe)]
        return unsafe { bcd::prim::entry::__n_compute() } != 0;
        #[cfg(not(target_os = "none"))]
        compute_pending()
    }

    // --- equals ---------------------------------------------------------

    /// `=`: the main total after `+`/`-`; else `true`: the second operand and
    /// the pending mul/div follow ([`Calculator::eq_operand`],
    /// [`Calculator::do_mul_div_equals`]).
    #[inline(never)]
    fn equals<P: Printer>(&mut self, out: &mut P) -> bool {
        if get(MODE) == ADD_SUB {
            // Recall and clear the main total.
            let rounded = Num::round_dp(T1, TR);
            out.number(T1, rounded_mark(rounded), SymB::Total);
            WR.copy_from(TR);
            SR.clear();
            TR.clear();
            set(MODE, NEW);
            return false;
        }
        // Multiply/divide result (or raise-to-power with repeated `=`).
        true
    }

    /// The `=` / `M=±` second-operand row (latches a typed entry first).
    #[inline(never)]
    fn eq_operand<P: Printer>(&mut self, out: &mut P) {
        let fresh = self.take_entry();
        self.print_operand(fresh, out);
    }

    /// Print the `=`/`M=±` second-operand row. A freshly typed entry prints at
    /// its entered scale. A repeated operand (no new entry) prints at the
    /// entered scale when the pending operation itself began from a typed entry
    /// (e.g. `8 * M=+` → `8`), otherwise rescaled to the current DP (e.g. a
    /// repeat chain `2 M+ * =` → `2.000000`).
    #[inline(always)]
    fn print_operand<P: Printer>(&self, fresh: bool, out: &mut P) {
        operand_shown(fresh || is(OP_FROM_ENTRY));
        out.number(T1, SymA::Equals, SymB::None);
    }

    /// Execute the pending mul/div for `=`/`%`/`M=±` into T2 (`frac` as for
    /// [`Num::div`]) and update the constant-operation state. Each copy below
    /// touches a register that is not an operand, so it may precede the
    /// operation.
    #[inline(always)]
    fn compute_mul_div(&mut self, frac: u8) -> bool {
        let op = get(MOP);
        if op >= OP_CONST_DIV {
            // DR = WR (the new operand); result = DR (op) CR.
            DR.copy_from(WR);
        } else if op == OP_DIV {
            // The divisor becomes the constant (const div).
            CR.copy_from(WR);
            set(MOP, OP_CONST_DIV);
        } else if op == OP_MUL {
            // After `=`, the multiplicand constant stays in CR (const mul).
            set(MOP, OP_CONST_MUL);
        }
        // No pending operation divides DR / WR (0/0 overflows).
        let b = if op == OP_MUL || op == OP_NONE {
            WR
        } else {
            CR
        };
        mul_or_div(op, DR, b, frac)
    }

    /// Run the pending mul/div for `=`/`%` and print the result row with
    /// `sym_b`. `pct` (from [`Calculator::percent`]): [`PCT`] asks for a
    /// floating quotient, [`PCT_X100`] also scales a divide x100 afterwards (a
    /// multiply's /100 was folded into the operand).
    #[inline(never)]
    fn do_mul_div_equals<P: Printer>(&mut self, out: &mut P, sym_b: SymB, pct: u8) {
        // `M=+`/`M=-` (sym_b): a floating quotient, WR = the raw result, and
        // the shown value accumulates into memory instead of becoming WR.
        let mem = sym_b == SymB::MemPlus || sym_b == SymB::MemMinus;
        if !self.compute_mul_div(if mem { FLOAT } else { div_frac(pct != 0) }) {
            self.overflow(out);
            return;
        }
        if mem {
            WR.copy_from(T2);
        }
        if pct == PCT_X100 {
            // (A percent quotient is floating: at most 13 significant digits
            // with 13 - integer digits places, so x100 keeps them 13, or ends
            // at scale 0: re-truncating to 13 significant digits never drops
            // one, and the M2 `trunc_sig` step is gone.)
            times_100(T2);
        }
        set(MODE, NEW);
        // Format with DP + rounding; the SHOWN (rounded) value becomes WR so a
        // following +/- or chain aligns to the printed number, not the raw
        // high-precision intermediate.
        // (width >= digits >= integer digits: one check covers both.)
        let rounded = shown_from_t2();
        if T1.width() > CAPACITY {
            self.overflow(out);
            return;
        }
        if mem {
            // Accumulate the rounded value into memory (matches the row).
            if !Num::add_signed(MR, MR, T1, sym_b == SymB::MemMinus) {
                self.overflow(out);
                return;
            }
            set(LAMP_MEMORY, !MR.is_zero() as u8);
        } else {
            WR.copy_from(T1);
        }
        out.number(T1, rounded_mark(rounded), sym_b);
    }

    // --- percent --------------------------------------------------------

    /// `%`: print the operand and fold /100 into a multiply; returns the
    /// percent code for [`Calculator::do_mul_div_equals`].
    #[inline(never)]
    fn percent<P: Printer>(&mut self, out: &mut P) -> u8 {
        self.take_entry();
        // The entered operand prints with `%`.
        out.number(WR, SymA::Percent, SymB::None);
        // For percent-multiply, the operand means "x (n/100)": fold /100 into
        // the operand before multiplying. For percent-divide, scale x100 after.
        let op = get(MOP);
        if op == OP_DIV || op == OP_CONST_DIV {
            return PCT_X100;
        }
        if op == OP_MUL || op == OP_CONST_MUL {
            WR.set_scale(WR.scale() + 2);
            zero_normalise(WR);
        }
        PCT
    }

    // --- exchange -------------------------------------------------------

    #[inline(never)]
    fn exchange<P: Printer>(&mut self, out: &mut P) {
        self.take_entry();
        // CR=WR, RR=DR, DR=WR, WR=RR  → swap WR and DR, CR tracks the new DR.
        out.number(WR, SymA::None, SymB::Exchange);
        T1.copy_from(DR);
        CR.copy_from(WR);
        DR.copy_from(WR);
        WR.copy_from(T1);
        set(MODE, NEW);
    }

    // --- subtotal / reference ------------------------------------------

    #[inline(never)]
    fn subtotal_key<P: Printer>(&mut self, out: &mut P) {
        if self.take_entry() {
            // Reference number / date: printed as typed, changes nothing.
            out.number(WR, SymA::None, SymB::Reference);
            return;
        }
        let mode = get(MODE);
        let mop = get(MOP);
        let shown = if mode == ADD_SUB {
            // A subtotal row never carries the rounded mark (the diamond and
            // the `^` are the same physical column).
            Num::round_dp(T1, SR);
            SR.clear();
            T1
        } else if mode == NEW && (mop == OP_MUL || mop == OP_DIV) {
            // Intermediate chain result: print the pending product/quotient.
            if !self.compute() {
                return;
            }
            WR.copy_from(T2);
            DR.copy_from(T2);
            CR.copy_from(T2);
            T2
        } else if mode == MUL_DIV {
            // After a chain `*`/`/` the running product is in DR; print it.
            DR
        } else {
            return;
        };
        out.number(shown, SymA::SubTotal, SymB::None);
    }

    // --- memory ---------------------------------------------------------

    #[inline(never)]
    fn memory_add<P: Printer>(&mut self, minus: bool, out: &mut P) {
        self.take_entry();
        // The ROM echoes a bare `M+`/`M-` key: it prints the current operand
        // with the memory-key glyph in symbol column A (spec A7), aligned to
        // the decimal selector like a `+`/`-` operand (10-1 `10.00`, 11-2
        // `2.000000`).
        operand_shown(false);
        let sym = if minus { SymA::MemMinus } else { SymA::MemPlus };
        out.number(T1, sym, SymB::None);
        if Num::add_signed(MR, MR, WR, minus) {
            set(LAMP_MEMORY, !MR.is_zero() as u8);
            set(MODE, NEW);
        } else {
            self.overflow(out);
        }
    }

    #[inline(never)]
    fn recall_memory<P: Printer>(&mut self, out: &mut P) {
        WR.copy_from(MR);
        set(ENTERING, 0);
        set(MODE, NEW);
        // RM prints `R` in column A and `M` in column B (spec A8); never the
        // rounded-up mark (a recall, not a result). A zero register prints as
        // the bare `0` the ROM shows (scale 0), not DP-padded.
        Num::round_dp(T1, MR);
        zero_as_bare(T1);
        out.number(T1, SymA::Recall, SymB::Mem);
    }

    #[inline(never)]
    fn clear_memory<P: Printer>(&mut self, out: &mut P) {
        // CM prints `C` in column A and `M` in column B (spec A8); a zero
        // register prints as the bare `0` (scale 0), matching the ROM.
        Num::round_dp(T1, MR);
        zero_as_bare(T1);
        out.number(T1, SymA::Clear, SymB::Mem);
        WR.copy_from(MR);
        MR.clear();
        set(LAMP_MEMORY, 0);
        set(ENTERING, 0);
        set(MODE, NEW);
    }

    // --- square root ----------------------------------------------------

    #[inline(never)]
    fn sqrt<P: Printer>(&mut self, out: &mut P) {
        let fresh = self.take_entry();
        // The radicand prints with the SQRT symbol: a freshly typed entry at
        // its entered scale, otherwise rescaled to the current DP.
        operand_shown(fresh);
        out.number(T1, SymA::Sqrt, SymB::None);

        // Root precision: FL fills to ~14 significant digits; fixed modes show
        // DP fractional digits (rounded per the switch).
        let int_q = WR.int_digits().div_ceil(2); // integer digits of the root
        let room = CAPACITY - int_q; // int_q <= 8
                                     // Fixed modes compute one guard digit, then round/truncate to DP.
        let fl = rounding() == Rounding::Float as u8;
        let frac = if !fl && dp() + 1 < room {
            dp() + 1
        } else {
            room
        };
        Num::sqrt_at(T2, WR, frac);
        if !fl {
            Num::round_dp(T2, T2);
        }
        WR.copy_from(T2);
        CR.copy_from(T2);
        set(MOP, OP_NONE);
        set(MODE, NEW);
        // Square-root result rows never carry the rounded-up mark.
        out.number(T2, SymA::Sqrt, SymB::None);
    }
}

/// The quotient's fractional digits for a result row: floating (13
/// significant digits) in FL or for percent, else DP + one guard digit so
/// `round_dp` rounds the last place correctly.
#[inline(always)]
fn div_frac(pct: bool) -> u8 {
    // ASSUMPTION (spec M3-K1): the DP selector reads 0..=8, so DP + 1 always
    // fits the register.
    if pct || rounding() == Rounding::Float as u8 {
        FLOAT
    } else {
        dp() + 1
    }
}

/// `T2 = a * b` for a multiply operation, else `T2 = a / b` to `frac`.
#[inline(always)]
fn mul_or_div(op: u8, a: Num, b: Num, frac: u8) -> bool {
    if op == OP_MUL || op == OP_CONST_MUL {
        Num::mul(T2, a, b)
    } else {
        Num::div(T2, a, b, frac)
    }
}

/// T1 = the value an operand-echo row prints (spec A6/M2-O1): WR as typed for
/// a fresh operand, else aligned to the decimal selector, exactly as the ROM's
/// operand print does (9-1 `11 ×` typed vs 10-1 `10.00 ×` after M+). `false`
/// when the alignment would lose digits (T1 is then WR unaligned).
#[inline(never)]
fn operand_shown(fresh: bool) -> bool {
    T1.copy_from(WR);
    fresh || T1.rescale(dp())
}

/// T1 = T2 formatted for a result row: as is in FL, else rounded/truncated to
/// DP. Returns whether it rounded up.
#[inline(always)]
fn shown_from_t2() -> bool {
    if rounding() == Rounding::Float as u8 {
        T1.copy_from(T2);
        false
    } else {
        Num::round_dp(T1, T2)
    }
}

/// A zero magnitude collapses to the bare `0` the ROM prints (scale 0, sign
/// +), instead of being padded to the decimal selector. The original register
/// holds a zero at scale 0, so `C`/`CM`/`RM` of an empty register print `0`,
/// while a `+`/`-` operand of zero keeps its padding (e.g. 14-3 `0.00000000 +`).
#[inline(never)]
fn zero_as_bare(n: Num) {
    if n.is_zero() {
        n.clear();
    }
}

/// Zero gets sign + and scale 0 (after a scale change).
#[inline(always)]
fn zero_normalise(n: Num) {
    if n.is_zero() {
        n.set_neg(false);
        n.set_scale(0);
    }
}

/// Symbol printed in column A when a `*`/`/` operand row is emitted.
#[inline(always)]
fn sym_for(op: u8) -> SymA {
    if op == OP_MUL || op == OP_CONST_MUL {
        SymA::Multiply
    } else {
        SymA::Divide
    }
}

/// Column A of a result row (total `*`, or `M+`/`M-` for `M=±`, in column
/// B): the rounded-up `^` mark when `rounded` (spec A8: e.g. `56.09 ^ *`,
/// `12.72 ^ M+`).
#[inline(always)]
fn rounded_mark(rounded: bool) -> SymA {
    if rounded {
        SymA::Rounded
    } else {
        SymA::None
    }
}

/// Multiply by 100 for percent: consume fractional digits first, then shift
/// the magnitude up.
#[inline(never)]
fn times_100(n: Num) {
    // (A shift that would lose digits leaves 15+ digits either way: the
    // caller's width check then reports overflow.)
    let s = n.scale();
    if s >= 2 {
        n.set_scale(s - 2);
    } else {
        n.shl10(2 - s);
        n.set_scale(0);
    }
    // (No zero normalisation: a zero quotient is already + at scale 0 and
    // stays so.)
}

/// Zero the working registers 0x00-0x70 (digits, sign, scale and the S3 flag)
/// except register `keep` (0x80: none): power-on, and `C` keeping memory.
#[inline(never)]
fn reset(keep: u8) {
    let mut r = 0;
    while r < 0x80 {
        if r != keep {
            Num(r).clear();
            set(r | 3, 0);
        }
        r += 0x10;
    }
}

/// `T2 = DR (op) WR` for the pending operation (the constant ones first move
/// DR = WR, WR = CR); `false` with none pending or on overflow. The host twin
/// of `__n_compute` (asm/num.s).
#[cfg(not(target_os = "none"))]
pub fn compute_pending() -> bool {
    let op = get(MOP);
    if op == OP_NONE {
        return false;
    }
    if op >= OP_CONST_DIV {
        DR.copy_from(WR);
        WR.copy_from(CR);
    }
    mul_or_div(op, DR, WR, FLOAT)
}
