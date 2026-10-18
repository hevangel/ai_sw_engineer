//! Digit-serial BCD numbers, the representation the original keeps in 4002
//! registers: 16 decimal digits (one per nibble), least significant first,
//! plus a sign and the number of fractional digits.
//!
//! Every operation works one digit at a time with `u8` carries, so the code
//! maps onto 4004 `ADD`/`SUB`/`DAA`-style loops without wider integers.

/// Digits per register (one 4002 register holds 16 main characters).
pub const DIGITS: usize = 16;
/// Printable / storable digits (manual: "14 digits plus decimal point and sign").
pub const CAPACITY: u8 = 14;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Num {
    /// `d[0]` is the least significant digit; each entry is 0..=9.
    pub d: [u8; DIGITS],
    pub neg: bool,
    /// Number of fractional digits (position of the decimal point).
    pub scale: u8,
}

impl Num {
    pub const ZERO: Num = Num {
        d: [0; DIGITS],
        neg: false,
        scale: 0,
    };

    pub fn is_zero(&self) -> bool {
        self.d.iter().all(|&x| x == 0)
    }

    /// Number of significant digits (0 for zero).
    pub fn digits(&self) -> u8 {
        let mut n = DIGITS;
        while n > 0 && self.d[n - 1] == 0 {
            n -= 1;
        }
        n as u8
    }

    /// Digits printed: significant digits, but never fewer than `scale + 1`
    /// (a leading `0` before the point).
    pub fn width(&self) -> u8 {
        let len = self.digits();
        if len > self.scale {
            len
        } else {
            self.scale + 1
        }
    }

    /// Magnitude comparison ignoring sign and scale (caller aligns first).
    pub fn cmp_mag(&self, other: &Num) -> core::cmp::Ordering {
        let mut i = DIGITS;
        while i > 0 {
            i -= 1;
            if self.d[i] != other.d[i] {
                return self.d[i].cmp(&other.d[i]);
            }
        }
        core::cmp::Ordering::Equal
    }

    /// Multiply by 10^k. Returns `false` (value unchanged) if digits would be lost.
    pub fn shl(&mut self, k: u8) -> bool {
        let k = k as usize;
        if k == 0 {
            return true;
        }
        if self.digits() as usize + k > DIGITS {
            return false;
        }
        let mut i = DIGITS;
        while i > 0 {
            i -= 1;
            self.d[i] = if i >= k { self.d[i - k] } else { 0 };
        }
        true
    }

    /// Divide by 10^k, truncating. Returns the most significant dropped digit
    /// and whether any dropped digit was nonzero.
    pub fn shr(&mut self, k: u8) -> (u8, bool) {
        let k = k as usize;
        if k == 0 {
            return (0, false);
        }
        let first = if k <= DIGITS { self.d[k - 1] } else { 0 };
        let mut any = false;
        for i in 0..DIGITS {
            if i < k {
                any |= self.d[i] != 0;
            }
            self.d[i] = if i + k < DIGITS { self.d[i + k] } else { 0 };
        }
        (first, any)
    }

    /// Add one unit in the last place to the magnitude. Returns `false` on carry out.
    pub fn inc_mag(&mut self) -> bool {
        for x in self.d.iter_mut() {
            if *x == 9 {
                *x = 0;
            } else {
                *x += 1;
                return true;
            }
        }
        false
    }

    /// Change the scale to `scale`, appending zeros or truncating.
    /// Returns (ok, first dropped digit, any dropped nonzero).
    pub fn rescale(&mut self, scale: u8) -> (bool, u8, bool) {
        if scale >= self.scale {
            let ok = self.shl(scale - self.scale);
            if ok {
                self.scale = scale;
            }
            (ok, 0, false)
        } else {
            let (first, any) = self.shr(self.scale - scale);
            self.scale = scale;
            (true, first, any)
        }
    }

    fn add_mag(&mut self, o: &Num) -> bool {
        let mut carry = 0u8;
        for i in 0..DIGITS {
            let mut s = self.d[i] + o.d[i] + carry;
            carry = 0;
            if s > 9 {
                s -= 10;
                carry = 1;
            }
            self.d[i] = s;
        }
        carry == 0
    }

    /// self.mag -= o.mag, requires self.mag >= o.mag.
    fn sub_mag(&mut self, o: &Num) {
        let mut borrow = 0u8;
        for i in 0..DIGITS {
            let sub = o.d[i] + borrow;
            if self.d[i] >= sub {
                self.d[i] -= sub;
                borrow = 0;
            } else {
                self.d[i] = self.d[i] + 10 - sub;
                borrow = 1;
            }
        }
    }

    /// Signed addition of `o` (with its sign flipped if `negate`).
    /// Both operands are brought to the larger scale. Returns `None` if the
    /// result does not fit `CAPACITY` digits; `self` is then unchanged.
    pub fn add_signed(&self, o: &Num, negate: bool) -> Option<Num> {
        let mut a = *self;
        let mut b = *o;
        b.neg ^= negate;
        let scale = if a.scale > b.scale { a.scale } else { b.scale };
        if !a.rescale(scale).0 || !b.rescale(scale).0 {
            return None;
        }
        if a.neg == b.neg {
            if !a.add_mag(&b) {
                return None;
            }
        } else if a.cmp_mag(&b) != core::cmp::Ordering::Less {
            a.sub_mag(&b);
        } else {
            b.sub_mag(&a);
            a = b;
        }
        if a.is_zero() {
            a.neg = false;
        }
        if a.width() > CAPACITY {
            return None;
        }
        Some(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(digits: &[u8], scale: u8, neg: bool) -> Num {
        // digits given most significant first
        let mut n = Num::ZERO;
        for (i, &x) in digits.iter().rev().enumerate() {
            n.d[i] = x;
        }
        n.scale = scale;
        n.neg = neg;
        n
    }

    #[test]
    fn add_and_sub_across_scales() {
        let a = num(&[1, 2, 3, 4], 2, false); // 12.34
        let b = num(&[3, 4, 5, 6, 0], 3, false); // 34.560
        let r = a.add_signed(&b, true).unwrap(); // 12.34 - 34.560 = -22.220
        assert_eq!(r, num(&[2, 2, 2, 2, 0], 3, true));
        let r = r.add_signed(&b, false).unwrap();
        assert_eq!(r, num(&[1, 2, 3, 4, 0], 3, false));
    }

    #[test]
    fn capacity_is_fourteen_digits() {
        let a = num(&[9, 0, 0, 0, 0, 0], 0, false);
        let mut a8 = a;
        assert!(a8.rescale(8).0); // 900000.00000000 = 14 digits
        let b = num(&[1, 0, 0, 0, 0, 0], 0, false);
        assert!(a8.add_signed(&b, false).is_none());
    }

    #[test]
    fn shr_reports_dropped_digits() {
        let mut a = num(&[5, 7, 9], 2, false);
        assert_eq!(a.rescale(1), (true, 9, true));
        assert_eq!(a, num(&[5, 7], 1, false));
    }
}
