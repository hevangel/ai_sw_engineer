//! Keys, selector switches and lamps as the firmware sees them.

/// The 30 populated keys. Discriminants are the keyboard-matrix codes the
/// oracle uses: `129 + 4 * row + column` (see platform/io_map.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Key {
    Cm = 129,
    Rm = 130,
    MMinus = 131,
    MPlus = 132,
    Sqrt = 133,
    Percent = 134,
    MEqMinus = 135,
    MEqPlus = 136,
    St = 137,
    Div = 138,
    Mul = 139,
    Eq = 140,
    Minus = 141,
    Plus = 142,
    D9 = 145,
    D6 = 146,
    D3 = 147,
    Point = 148,
    D8 = 149,
    D5 = 150,
    D2 = 151,
    D00 = 152,
    D7 = 153,
    D4 = 154,
    D1 = 155,
    D0 = 156,
    Sign = 157,
    Ex = 158,
    Ce = 159,
    C = 160,
}

impl Key {
    // Plain comparisons, not `RangeInclusive::contains`: on the 4004 the
    // latter stays an out-of-line call of ~290 bytes.
    /// Decode a matrix code; `None` for the two unpopulated positions (143,
    /// 144) and anything outside 129..=160. Arithmetic only, so no lookup
    /// table reaches the 4004 image.
    pub fn from_code(code: u8) -> Option<Key> {
        if !(129..=160).contains(&code) || code == 143 || code == 144 {
            return None;
        }
        // SAFETY: `Key` is `repr(u8)` and every code in 129..=160 except
        // 143/144 is a declared discriminant.
        Some(unsafe { core::mem::transmute::<u8, Key>(code) })
    }

    /// Digit value for `0`–`9`. The digit keys sit in matrix rows 4..6
    /// (codes 145..156): column 0/1/2 of row r is `9 - r - 3*column`, and `0`
    /// is code 156 (column 3 carries `.`, `00`).
    pub fn digit(self) -> Option<u8> {
        let code = self as u8;
        if code == Key::D0 as u8 {
            return Some(0);
        }
        if !(145..=155).contains(&code) {
            return None;
        }
        let k = code - 145;
        let col = k & 3;
        if col == 3 {
            return None;
        }
        Some(9 - (k >> 2) - col - col - col)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Rounding {
    /// FL: floating decimal point.
    Float = 0,
    /// 5/4: round half up.
    Round = 1,
    /// IN: truncate.
    Truncate = 2,
}

impl Rounding {
    /// Selector value read from keyboard row 9: 0 = FL, 1 = 5/4, 8 = IN.
    pub fn from_selector(v: u8) -> Option<Rounding> {
        match v {
            0 => Some(Rounding::Float),
            1 => Some(Rounding::Round),
            8 => Some(Rounding::Truncate),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switches {
    /// Decimal selector: 0, 1, 2, 3, 4, 5, 6 or 8.
    pub dp: u8,
    pub rounding: Rounding,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lamps {
    pub memory: bool,
    pub overflow: bool,
    pub negative: bool,
}
