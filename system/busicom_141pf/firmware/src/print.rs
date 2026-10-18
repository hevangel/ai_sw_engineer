//! Printed rows: what the 20 hammers put on one line of paper.

use crate::bcd::Num;

/// Numeric columns on the drum printer (14 digits + decimal point).
pub const COLUMNS: usize = 15;

/// Function symbol printed in symbol columns A/B.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    None,
    Plus,
    Minus,
    /// Total star.
    Total,
    /// Subtotal diamond.
    SubTotal,
    /// `#` after number entry + ST (reference / date).
    Reference,
    Multiply,
    Divide,
    Equals,
    Exchange,
    Percent,
    MemPlus,
    MemMinus,
    ClearMemory,
    RecallMemory,
    Sqrt,
    Clear,
}

impl Symbol {
    /// The oracle JSON's `symbol` string.
    pub fn as_str(self) -> &'static str {
        match self {
            Symbol::None => "",
            Symbol::Plus => "+",
            Symbol::Minus => "-",
            Symbol::Total => "*",
            Symbol::SubTotal => "ST",
            Symbol::Reference => "#",
            Symbol::Multiply => "*op",
            Symbol::Divide => "/",
            Symbol::Equals => "=",
            Symbol::Exchange => "EX",
            Symbol::Percent => "%",
            Symbol::MemPlus => "M+",
            Symbol::MemMinus => "M-",
            Symbol::ClearMemory => "CM",
            Symbol::RecallMemory => "RM",
            Symbol::Sqrt => "SQRT",
            Symbol::Clear => "C",
        }
    }
}

/// One printed line. `cols[0]` is the leftmost numeric column; each holds
/// b' ', b'0'..=b'9' or b'.'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub cols: [u8; COLUMNS],
    pub symbol: Symbol,
    pub red: bool,
    pub rounded: bool,
    pub overflow: bool,
}

impl Row {
    pub const fn blank(symbol: Symbol) -> Row {
        Row {
            cols: [b' '; COLUMNS],
            symbol,
            red: false,
            rounded: false,
            overflow: false,
        }
    }

    /// The dotted line printed on overflow.
    pub const fn overflow() -> Row {
        Row {
            cols: [b'.'; COLUMNS],
            symbol: Symbol::None,
            red: false,
            rounded: false,
            overflow: true,
        }
    }

    /// Right-aligned number: integer digits without leading zeros (at least
    /// one), then `.` and `n.scale` fractional digits when `scale > 0`.
    pub fn number(n: &Num, symbol: Symbol) -> Row {
        let mut row = Row::blank(symbol);
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
}

/// Where rows go: the drum-printer driver on the target, a recorder on the host.
pub trait Printer {
    fn print(&mut self, row: &Row);
}
