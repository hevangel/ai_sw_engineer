//! Printed rows: what the 20 hammers put on one line of paper.
//!
//! The Model-102 drum fires TWO independent symbol columns (A and B) beside the
//! 15 numeric columns (ai_hw_engineer board spec §4.2: hammer-word bit 0 =
//! symbol column A, bit 1 = symbol column B). The original ROM uses both: a
//! `CM` row prints `C` in column A and `M` in column B, an `RM` row prints `R`
//! in A and `M` in B, an `M=±` result prints its `M+`/`M-` in column B while a
//! bare `M+`/`M-` key echo prints in column A, and a rounded result prints the
//! `^` mark in column A beside the result symbol in column B. A single-glyph
//! row model cannot represent this, so [`Row`] carries one glyph per column
//! ([`SymA`] / [`SymB`]); the rounded `^` mark IS column A sector 7, so it is
//! the [`SymA::Rounded`] variant rather than a separate flag.
//!
//! Each glyph's discriminant is its drum sector, so the printer driver never
//! needs a table. The numeric columns are not stored in the row: formatting a
//! number writes, for each of the 15 columns, the drum sector that column
//! fires at (digit `d` -> sector `d`, `.` -> 10, blank -> [`BLANK`]) into the
//! print register [`PR`] in 4002 RAM (issue #18, B5). One line is printed at a
//! time, so one register is enough.

use crate::bcd::Num;
use crate::ram::{rd, srd, swr, wr};

/// Numeric columns on the drum printer (14 digits + decimal point).
pub const COLUMNS: usize = 15;
/// [`COLUMNS`] as the `u8` the firmware loops use.
const NCOLS: u8 = COLUMNS as u8;
/// The print register: character `c` = drum sector of numeric column `c`
/// (0 = leftmost).
pub const PR: u8 = 0xE0;
/// Column value for "no hammer" (never equal to a sector 0..12).
pub const BLANK: u8 = 15;
/// Drum sector of `.` in a numeric column.
pub const POINT: u8 = 10;

/// Symbol **column A** glyph (hammer-word bit 0); discriminant = drum sector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SymA {
    /// `◇` subtotal / diamond.
    SubTotal = 0,
    Plus = 1,
    Minus = 2,
    /// `×` multiply operand.
    Multiply = 3,
    /// `÷` divide operand.
    Divide = 4,
    /// `M+` bare memory-add key echo.
    MemPlus = 5,
    /// `M-` bare memory-subtract key echo.
    MemMinus = 6,
    /// `^` rounded-up mark.
    Rounded = 7,
    /// `=` second-operand echo.
    Equals = 8,
    /// `√` square root.
    Sqrt = 9,
    Percent = 10,
    /// `C` clear.
    Clear = 11,
    /// `R` recall memory.
    Recall = 12,
    None = BLANK,
}

/// Symbol **column B** glyph (hammer-word bit 1); discriminant = drum sector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SymB {
    /// `#` reference / date.
    Reference = 0,
    /// `*` total star.
    Total = 1,
    /// `M+` memory-equals result.
    MemPlus = 5,
    /// `M-` memory-equals result.
    MemMinus = 6,
    /// `Ex` exchange.
    Exchange = 10,
    /// `M` clear/recall-memory companion glyph.
    Mem = 12,
    None = BLANK,
}

impl SymA {
    /// The glyph as the original-ROM trace prints it, or `""` for [`SymA::None`].
    pub fn glyph(self) -> &'static str {
        match self {
            SymA::None => "",
            SymA::SubTotal => "◇",
            SymA::Plus => "+",
            SymA::Minus => "-",
            SymA::Multiply => "×",
            SymA::Divide => "÷",
            SymA::MemPlus => "M+",
            SymA::MemMinus => "M-",
            SymA::Rounded => "^",
            SymA::Equals => "=",
            SymA::Sqrt => "√",
            SymA::Percent => "%",
            SymA::Clear => "C",
            SymA::Recall => "R",
        }
    }
}

impl SymB {
    /// The glyph as the original-ROM trace prints it, or `""` for [`SymB::None`].
    pub fn glyph(self) -> &'static str {
        match self {
            SymB::None => "",
            SymB::Reference => "#",
            SymB::Total => "*",
            SymB::MemPlus => "M+",
            SymB::MemMinus => "M-",
            SymB::Exchange => "Ex",
            SymB::Mem => "M",
        }
    }
}

/// Print-register status characters: column-A sector, column-B sector, flags.
const PR_SYM_A: u8 = PR;
const PR_SYM_B: u8 = PR | 1;
const PR_FLAGS: u8 = PR | 2;
/// `PR_FLAGS` bits.
const FLAG_RED: u8 = 1;
const FLAG_OVERFLOW: u8 = 2;

/// One printed line, held entirely in the print register [`PR`]: the 15
/// column sectors in its main characters, the two symbol sectors and the line
/// flags in its status characters. A `Row` value is only a token for that
/// register; a row is printed before the next one is formatted.
#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    _pr: (),
}

/// Fill all 15 columns of [`PR`] with `v`.
#[inline(never)]
fn fill(v: u8) {
    let mut c = 0;
    while c < NCOLS {
        wr(PR + c, v);
        c += 1;
    }
}

/// Set the symbol columns and flags of the row in [`PR`].
#[inline(never)]
fn finish(sym_a: SymA, sym_b: SymB, flags: u8) -> Row {
    swr(PR_SYM_A, sym_a as u8);
    swr(PR_SYM_B, sym_b as u8);
    swr(PR_FLAGS, flags);
    Row { _pr: () }
}

impl Row {
    /// Blank number columns with the given symbols.
    #[inline(always)]
    pub fn blank(sym_a: SymA, sym_b: SymB) -> Row {
        fill(BLANK);
        finish(sym_a, sym_b, 0)
    }

    /// The dotted line printed on overflow (no symbols).
    #[inline(always)]
    pub fn overflow() -> Row {
        fill(POINT);
        finish(SymA::None, SymB::None, FLAG_OVERFLOW)
    }

    /// Right-aligned number with the given symbol columns: integer digits
    /// without leading zeros (at least one), then `.` and `n.scale` fractional
    /// digits when `scale > 0`.
    ///
    /// On the target the row is built by `__pr_row` (asm/drivers.s, through
    /// [`Printer::number`]); this is its twin. ASSUMPTION: a printed number has
    /// width <= 14 and scale <= 13 (it is rounded to the DP or to FL's 13
    /// digits first), so it fits the 15 columns; `__pr_row` takes both mod 16.
    pub fn number(n: Num, sym_a: SymA, sym_b: SymB) -> Row {
        fill(BLANK);
        let scale = n.scale();
        let width = n.width();
        let mut col = NCOLS;
        let mut i = 0;
        while i < width {
            if i == scale && scale > 0 {
                col -= 1;
                wr(PR + col, POINT);
            }
            col -= 1;
            wr(PR + col, n.digit(i));
            i += 1;
        }
        finish(sym_a, sym_b, if n.is_neg() { FLAG_RED } else { 0 })
    }

    /// Convenience: a number row with only a column-A glyph.
    #[inline(always)]
    pub fn num_a(n: Num, sym_a: SymA) -> Row {
        Row::number(n, sym_a, SymB::None)
    }

    /// Convenience: a number row with only a column-B glyph.
    #[inline(always)]
    pub fn num_b(n: Num, sym_b: SymB) -> Row {
        Row::number(n, SymA::None, sym_b)
    }

    /// Drum sector of numeric column `c` (0 = leftmost), [`BLANK`] if none.
    #[inline(always)]
    pub fn col_sector(c: u8) -> u8 {
        rd(PR + c)
    }

    /// Drum sector of symbol column A ([`BLANK`] for none).
    #[inline(always)]
    pub fn sym_a_sector(&self) -> u8 {
        srd(PR_SYM_A)
    }

    /// Drum sector of symbol column B ([`BLANK`] for none).
    #[inline(always)]
    pub fn sym_b_sector(&self) -> u8 {
        srd(PR_SYM_B)
    }

    /// Print in red (a negative number).
    #[inline(always)]
    pub fn red(&self) -> bool {
        srd(PR_FLAGS) & FLAG_RED != 0
    }

    /// The dotted overflow line.
    #[inline(always)]
    pub fn overflow_line(&self) -> bool {
        srd(PR_FLAGS) & FLAG_OVERFLOW != 0
    }

    /// Column-A glyph.
    pub fn sym_a(&self) -> SymA {
        SymA::from_sector(self.sym_a_sector())
    }

    /// Column-B glyph.
    pub fn sym_b(&self) -> SymB {
        SymB::from_sector(self.sym_b_sector())
    }

    /// The numeric columns as text (`' '`, `'0'..='9'`, `'.'`), for tests and
    /// the host tape.
    pub fn cols(&self) -> [u8; COLUMNS] {
        let mut out = [b' '; COLUMNS];
        for (c, slot) in out.iter_mut().enumerate() {
            *slot = match Row::col_sector(c as u8) {
                d @ 0..=9 => b'0' + d,
                POINT => b'.',
                _ => b' ',
            };
        }
        out
    }

    /// Whether the row carries the rounded-up `^` mark (column A sector 7).
    pub fn rounded(&self) -> bool {
        self.sym_a() == SymA::Rounded
    }

    /// The single collapsed token the scanned-manual JSON oracle
    /// (`oracle/manual_examples.json`) uses for this row's symbol. The oracle
    /// predates the two-column model and records one token per row, so the
    /// JSON-backed tests (`manual_examples.rs`, `port_level.rs`) fold the two
    /// columns back to it. The rounded `^` mark is reported separately via
    /// [`Row::rounded`], never as the token.
    pub fn oracle_symbol(&self) -> &'static str {
        if self.overflow_line() {
            return "overflow";
        }
        match (self.sym_a(), self.sym_b()) {
            (SymA::Clear, SymB::Mem) => "CM",
            (SymA::Recall, SymB::Mem) => "RM",
            (_, SymB::Total) => "*",
            (_, SymB::MemPlus) => "M+",
            (_, SymB::MemMinus) => "M-",
            (_, SymB::Reference) => "#",
            (_, SymB::Exchange) => "EX",
            (_, SymB::Mem) => "M",
            (SymA::SubTotal, _) => "ST",
            (SymA::Plus, _) => "+",
            (SymA::Minus, _) => "-",
            (SymA::Multiply, _) => "*op",
            (SymA::Divide, _) => "/",
            (SymA::MemPlus, _) => "M+",
            (SymA::MemMinus, _) => "M-",
            (SymA::Equals, _) => "=",
            (SymA::Sqrt, _) => "SQRT",
            (SymA::Percent, _) => "%",
            (SymA::Clear, _) => "C",
            (SymA::Recall, _) => "R",
            (SymA::Rounded, _) | (SymA::None, SymB::None) => "",
        }
    }
}

impl SymA {
    /// The glyph struck at drum sector `s` of column A ([`BLANK`] = none).
    pub fn from_sector(s: u8) -> SymA {
        assert!(s <= 12 || s == BLANK, "bad column-A sector {s}");
        // SAFETY: `SymA` is `repr(u8)` with discriminants 0..=12 and BLANK.
        unsafe { core::mem::transmute::<u8, SymA>(s) }
    }
}

impl SymB {
    /// The glyph struck at drum sector `s` of column B ([`BLANK`] = none).
    pub fn from_sector(s: u8) -> SymB {
        match s {
            0 => SymB::Reference,
            1 => SymB::Total,
            5 => SymB::MemPlus,
            6 => SymB::MemMinus,
            10 => SymB::Exchange,
            12 => SymB::Mem,
            _ => SymB::None,
        }
    }
}

/// Where rows go: the drum-printer driver on the target, a recorder on the host.
pub trait Printer {
    fn print(&mut self, row: &Row);

    /// Print a number row ([`Row::number`]). The target's printer overrides
    /// these three with one `__pr_row` call each (row build + drum print).
    #[inline(always)]
    fn number(&mut self, n: Num, sym_a: SymA, sym_b: SymB) {
        self.print(&Row::number(n, sym_a, sym_b))
    }

    /// Print a blank-value row ([`Row::blank`]).
    #[inline(always)]
    fn blank(&mut self, sym_a: SymA, sym_b: SymB) {
        self.print(&Row::blank(sym_a, sym_b))
    }

    /// Print the overflow row ([`Row::overflow`]).
    #[inline(always)]
    fn overflow(&mut self) {
        self.print(&Row::overflow())
    }
}
