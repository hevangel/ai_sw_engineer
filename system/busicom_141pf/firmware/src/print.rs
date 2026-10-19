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
//! The drum sector each glyph prints at is in `driver_printer` (`DRUM_SYMBOL_A`
//! / `DRUM_SYMBOL_B`); these enums name the glyph, the driver maps it to a
//! sector, so the core never encodes drum geometry.

use crate::bcd::Num;

/// Numeric columns on the drum printer (14 digits + decimal point).
pub const COLUMNS: usize = 15;

/// Symbol **column A** glyph (hammer-word bit 0). The drum sector of each
/// variant is fixed by the Model-102 drum (`driver_printer::DRUM_SYMBOL_A`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymA {
    None,
    /// `◇` subtotal / diamond (sector 0).
    SubTotal,
    /// `+` (sector 1).
    Plus,
    /// `-` (sector 2).
    Minus,
    /// `×` multiply operand (sector 3).
    Multiply,
    /// `÷` divide operand (sector 4).
    Divide,
    /// `M+` bare memory-add key echo (sector 5).
    MemPlus,
    /// `M-` bare memory-subtract key echo (sector 6).
    MemMinus,
    /// `^` rounded-up mark (sector 7).
    Rounded,
    /// `=` second-operand echo (sector 8).
    Equals,
    /// `√` square root (sector 9).
    Sqrt,
    /// `%` percent (sector 10).
    Percent,
    /// `C` clear (sector 11).
    Clear,
    /// `R` recall memory (sector 12).
    Recall,
}

/// Symbol **column B** glyph (hammer-word bit 1). The drum sector of each
/// variant is fixed by the Model-102 drum (`driver_printer::DRUM_SYMBOL_B`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymB {
    None,
    /// `#` reference / date (sector 0).
    Reference,
    /// `*` total star (sector 1).
    Total,
    /// `M+` memory-equals result (sector 5).
    MemPlus,
    /// `M-` memory-equals result (sector 6).
    MemMinus,
    /// `Ex` exchange (sector 10).
    Exchange,
    /// `M` clear/recall-memory companion glyph (sector 12).
    Mem,
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

/// One printed line. `cols[0]` is the leftmost numeric column; each holds
/// b' ', b'0'..=b'9' or b'.'. The two symbol columns are independent; the
/// rounded mark is `sym_a == SymA::Rounded`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub cols: [u8; COLUMNS],
    pub sym_a: SymA,
    pub sym_b: SymB,
    pub red: bool,
    pub overflow: bool,
}

impl Row {
    pub const fn blank(sym_a: SymA, sym_b: SymB) -> Row {
        Row {
            cols: [b' '; COLUMNS],
            sym_a,
            sym_b,
            red: false,
            overflow: false,
        }
    }

    /// The dotted line printed on overflow (no symbols).
    pub const fn overflow() -> Row {
        Row {
            cols: [b'.'; COLUMNS],
            sym_a: SymA::None,
            sym_b: SymB::None,
            red: false,
            overflow: true,
        }
    }

    /// Right-aligned number with the given symbol columns: integer digits
    /// without leading zeros (at least one), then `.` and `n.scale` fractional
    /// digits when `scale > 0`.
    pub fn number(n: &Num, sym_a: SymA, sym_b: SymB) -> Row {
        let mut row = Row::blank(sym_a, sym_b);
        row.red = n.neg;
        let mut col = COLUMNS;
        for i in 0..n.width() as usize {
            if i == n.scale as usize && n.scale > 0 {
                col -= 1;
                row.cols[col] = b'.';
            }
            col -= 1;
            row.cols[col] = b'0' + n.d[i];
        }
        row
    }

    /// Convenience: a number row with only a column-A glyph.
    pub fn num_a(n: &Num, sym_a: SymA) -> Row {
        Row::number(n, sym_a, SymB::None)
    }

    /// Convenience: a number row with only a column-B glyph.
    pub fn num_b(n: &Num, sym_b: SymB) -> Row {
        Row::number(n, SymA::None, sym_b)
    }

    /// Whether the row carries the rounded-up `^` mark (column A sector 7).
    pub fn rounded(&self) -> bool {
        self.sym_a == SymA::Rounded
    }

    /// The single collapsed token the scanned-manual JSON oracle
    /// (`oracle/manual_examples.json`) uses for this row's symbol. The oracle
    /// predates the two-column model and records one token per row, so the
    /// JSON-backed tests (`manual_examples.rs`, `port_level.rs`) fold the two
    /// columns back to it. The mapping is the inverse of how the core sets the
    /// columns: `CM`/`RM` (C+M / R+M) collapse to `CM`/`RM`, a column-B
    /// memory-equals result to `M+`/`M-`, the total star to `*`, and a bare
    /// column-A glyph to itself. The rounded `^` mark is reported separately
    /// via [`Row::rounded`], never as the token.
    pub fn oracle_symbol(&self) -> &'static str {
        if self.overflow {
            return "overflow";
        }
        match (self.sym_a, self.sym_b) {
            // Two-column rows → the oracle's single collapsed token.
            (SymA::Clear, SymB::Mem) => "CM",
            (SymA::Recall, SymB::Mem) => "RM",
            // Column-B-only result/echo rows.
            (_, SymB::Total) => "*",
            (_, SymB::MemPlus) => "M+",
            (_, SymB::MemMinus) => "M-",
            (_, SymB::Reference) => "#",
            (_, SymB::Exchange) => "EX",
            (_, SymB::Mem) => "M",
            // Column-A rows (the rounded mark is not a token on its own).
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

/// Where rows go: the drum-printer driver on the target, a recorder on the host.
pub trait Printer {
    fn print(&mut self, row: &Row);
}
