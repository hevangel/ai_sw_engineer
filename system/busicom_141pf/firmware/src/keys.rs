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
    /// Decode a matrix code; `None` for the two unpopulated positions.
    pub fn from_code(code: u8) -> Option<Key> {
        use Key::*;
        const TABLE: [Option<Key>; 32] = [
            Some(Cm),
            Some(Rm),
            Some(MMinus),
            Some(MPlus),
            Some(Sqrt),
            Some(Percent),
            Some(MEqMinus),
            Some(MEqPlus),
            Some(St),
            Some(Div),
            Some(Mul),
            Some(Eq),
            Some(Minus),
            Some(Plus),
            None,
            None,
            Some(D9),
            Some(D6),
            Some(D3),
            Some(Point),
            Some(D8),
            Some(D5),
            Some(D2),
            Some(D00),
            Some(D7),
            Some(D4),
            Some(D1),
            Some(D0),
            Some(Sign),
            Some(Ex),
            Some(Ce),
            Some(C),
        ];
        if (129..=160).contains(&code) {
            TABLE[(code - 129) as usize]
        } else {
            None
        }
    }

    /// Digit value for `0`–`9`.
    pub fn digit(self) -> Option<u8> {
        use Key::*;
        Some(match self {
            D0 => 0,
            D1 => 1,
            D2 => 2,
            D3 => 3,
            D4 => 4,
            D5 => 5,
            D6 => 6,
            D7 => 7,
            D8 => 8,
            D9 => 9,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    /// FL: floating decimal point.
    Float,
    /// 5/4: round half up.
    Round,
    /// IN: truncate.
    Truncate,
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
