//! Digit-serial BCD numbers, the representation the original keeps in 4002
//! registers: 16 decimal digits (one per nibble), least significant first,
//! plus a sign and the number of fractional digits.
//!
//! Every operation works one digit at a time with `u8` carries, so the code
//! maps onto 4004 `ADD`/`SUB`/`DAA`-style loops without wider integers. The
//! multiply/divide/sqrt helpers use fixed-size `u8` scratch buffers (products
//! of two 14-digit numbers need up to 28 digits); none use `u16`+ arithmetic
//! for digit values.

/// Digits per register (one 4002 register holds 16 main characters).
pub const DIGITS: usize = 16;
/// Bytes backing the packed digit array: two BCD digits (nibbles) per byte.
/// This matches the 4002's nibble density — the original ROM stores one digit
/// per 4-bit character, so one digit per byte would waste 2× the board RAM.
const PACKED: usize = DIGITS / 2;
/// Printable / storable digits (manual: "14 digits plus decimal point and sign").
pub const CAPACITY: u8 = 14;
/// Floating multiply/divide significant-digit budget (recovered-ROM behaviour).
pub const FLOAT_SIG: u8 = 13;

/// Width of the scratch magnitude buffers used by mul/div/sqrt.
const WIDE: usize = 34;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Num {
    /// Digits least-significant first, **two per byte** (nibble `2*i` = digit
    /// `i`): the low nibble of `d[i/2]` is digit `i` when `i` is even, the high
    /// nibble when `i` is odd. Access through [`Num::digit`] / [`Num::set_digit`]
    /// rather than indexing — the packed layout is the RAM-fit representation
    /// (see `report/final_report.md` M4). Each nibble holds 0..=9.
    d: [u8; PACKED],
    pub neg: bool,
    /// Number of fractional digits (position of the decimal point).
    pub scale: u8,
}

impl Num {
    pub const ZERO: Num = Num {
        d: [0; PACKED],
        neg: false,
        scale: 0,
    };

    /// Digit `i` (0 = least significant), 0..=9. `i` must be `< DIGITS`.
    #[inline]
    pub fn digit(&self, i: usize) -> u8 {
        let byte = self.d[i >> 1];
        if i & 1 == 0 {
            byte & 0x0F
        } else {
            byte >> 4
        }
    }

    /// Set digit `i` (0 = least significant) to `v` (0..=9). `i < DIGITS`.
    #[inline]
    pub fn set_digit(&mut self, i: usize, v: u8) {
        let idx = i >> 1;
        let byte = self.d[idx];
        self.d[idx] = if i & 1 == 0 {
            (byte & 0xF0) | (v & 0x0F)
        } else {
            (byte & 0x0F) | (v << 4)
        };
    }

    pub fn is_zero(&self) -> bool {
        crate::regops::is_zero(&self.d)
    }

    /// Number of significant digits (0 for zero).
    pub fn digits(&self) -> u8 {
        let mut n = DIGITS;
        while n > 0 && self.digit(n - 1) == 0 {
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

    /// Integer-part digit count (digits left of the point). `0` when `|self|<1`.
    pub fn int_digits(&self) -> u8 {
        self.digits().saturating_sub(self.scale)
    }

    /// Magnitude comparison ignoring sign and scale (caller aligns first).
    pub fn cmp_mag(&self, other: &Num) -> core::cmp::Ordering {
        crate::regops::cmp(&self.d, &other.d)
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
            self.set_digit(i, if i >= k { self.digit(i - k) } else { 0 });
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
        let first = if k <= DIGITS { self.digit(k - 1) } else { 0 };
        let mut any = false;
        for i in 0..DIGITS {
            if i < k {
                any |= self.digit(i) != 0;
            }
            self.set_digit(i, if i + k < DIGITS { self.digit(i + k) } else { 0 });
        }
        (first, any)
    }

    /// Add one unit in the last place to the magnitude. Returns `false` on carry out.
    pub fn inc_mag(&mut self) -> bool {
        for i in 0..DIGITS {
            let x = self.digit(i);
            if x == 9 {
                self.set_digit(i, 0);
            } else {
                self.set_digit(i, x + 1);
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
        crate::regops::add(&mut self.d, &o.d)
    }

    /// self.mag -= o.mag, requires self.mag >= o.mag.
    fn sub_mag(&mut self, o: &Num) {
        crate::regops::sub(&mut self.d, &o.d)
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

    /// Build a `Num` from a wide little-endian magnitude with the given scale,
    /// sign, taking digit buffer index `i` as power `i - scale`. Returns `None`
    /// if the magnitude needs more than `DIGITS` positions (register overflow).
    fn from_mag(buf: &[u8; WIDE], scale: u8, neg: bool) -> Option<Num> {
        for &x in buf.iter().skip(DIGITS) {
            if x != 0 {
                return None;
            }
        }
        let mut n = Num::ZERO;
        for (i, &v) in buf.iter().enumerate().take(DIGITS) {
            n.set_digit(i, v);
        }
        n.scale = scale;
        n.neg = neg;
        if n.is_zero() {
            n.neg = false;
            n.scale = 0;
        }
        Some(n)
    }

    /// Exact product. The natural scale is the sum of the operand scales.
    /// Returns `None` if the magnitude exceeds the register (never for two
    /// 14-digit operands whose product fits 14 digits — the caller still
    /// range-checks the printable width).
    pub fn mul(&self, o: &Num) -> Option<Num> {
        let mut acc = [0u8; WIDE];
        for i in 0..DIGITS {
            let di = self.digit(i);
            if di == 0 {
                continue;
            }
            let mut carry = 0u8;
            for j in 0..DIGITS {
                let pos = i + j;
                let oj = o.digit(j);
                if pos >= WIDE {
                    if oj != 0 {
                        return None;
                    }
                    continue;
                }
                let p = di * oj + acc[pos] + carry;
                acc[pos] = p % 10;
                carry = p / 10;
            }
            let mut pos = i + DIGITS;
            while carry > 0 {
                if pos >= WIDE {
                    return None;
                }
                let p = acc[pos] + carry;
                acc[pos] = p % 10;
                carry = p / 10;
                pos += 1;
            }
        }
        Num::from_mag(&acc, self.scale + o.scale, self.neg != o.neg)
    }

    /// Floating quotient `self / o`, truncated to [`FLOAT_SIG`] significant
    /// digits (the recovered-ROM full-precision division result). The integer
    /// part is always kept. Returns `None` on divide-by-zero.
    ///
    /// ASSUMPTION (spec M2-D1): FL division keeps 13 significant digits,
    /// truncated. Verified against the recovered ROM for examples 3-1, 3-4, 4-2.
    pub fn div_float(&self, o: &Num) -> Option<Num> {
        self.divide(o, None)
    }

    /// Quotient `self / o` truncated to exactly `frac` fractional digits
    /// (fixed-mode display); the caller applies rounding via [`Num::round_at`]
    /// — pass `frac + 1` guard digits when rounding is needed.
    pub fn div_fixed(&self, o: &Num, frac: u8) -> Option<Num> {
        self.divide(o, Some(frac))
    }

    /// Core long division (restoring, one BCD digit per position). `frac`:
    /// `None` → floating 13-significant-digit result; `Some(n)` → exactly `n`
    /// fractional digits, truncated.
    fn divide(&self, o: &Num, frac: Option<u8>) -> Option<Num> {
        if o.is_zero() {
            return None;
        }
        if self.is_zero() {
            return Some(Num::ZERO);
        }
        // Integer long division. Scale both operands to integers by dropping
        // their decimal points; the quotient's point is recovered from the
        // scale difference plus the number of extra fractional digits we grind
        // out. Dividend and divisor are little-endian integer magnitudes.
        //
        //   value(self) = N * 10^-self.scale,  value(o) = D * 10^-o.scale
        //   value(self)/value(o) = (N/D) * 10^(o.scale - self.scale)
        //
        // We compute N/D to `gen` extra fractional digits by appending zeros to
        // N, giving an integer quotient Q with
        //   quotient_value = Q * 10^(o.scale - self.scale - gen).
        let n_digits = self.digits() as usize;
        let d_digits = o.digits() as usize;

        // Leading-digit power of the exact quotient (approx, never too low).
        let num_exp = n_digits as i16 - 1 - self.scale as i16;
        let den_exp = d_digits as i16 - 1 - o.scale as i16;
        let lead_hi = num_exp - den_exp + 1;

        // Fractional digits of the final result.
        //
        // Fixed mode: exactly `frac`. Floating mode: generate enough fractional
        // digits to always cover 13 significant digits regardless of the true
        // integer size, then trim to 13 significant digits afterwards.
        let gen_frac: u8 = match frac {
            Some(n) => n,
            None => {
                // The integer part has at most `max(lead_hi,1)` digits; we grind
                // down to power -(FLOAT_SIG) which is always deep enough.
                let depth = FLOAT_SIG as i16 - lead_hi.min(0); // >= FLOAT_SIG
                depth.max(FLOAT_SIG as i16) as u8
            }
        };
        // Grind `gen_frac` fractional digits plus one guard digit.
        let gen = gen_frac as i16 + 1;

        // Build dividend integer N (little-endian) and divisor integer D.
        // N occupies digits [0, n_digits); shift it up by `gen + extra` so the
        // quotient has enough low digits. The quotient's lowest power is
        //   q_low_power = (o.scale as i16 - self.scale as i16 - gen) ... in
        // units of 10. We align via `point_shift` below.
        let (mut divd, divs, divs_hi) =
            match build_div_operands(self, o, gen, n_digits, d_digits) {
                Some(x) => x,
                None => return None,
            };

        // Quotient power of the final result: value = Q * 10^qp where
        //   qp = (o.scale - self.scale) - gen  (since N was scaled by 10^gen)
        // and the printed scale is `frac_out`. Produce quotient digit at buffer
        // index k (little-endian), representing Q's digit k.
        //
        // Long division: for position from high to low, count subtractions of
        // divisor shifted left by that position.
        let dividend_hi = n_digits + gen.max(0) as usize; // one past top is fine
        let mut quo_int = restoring_divide(&mut divd, &divs, divs_hi, dividend_hi);

        // Q value = quo_int (little-endian integer). The quotient value is
        //   Q * 10^qp,  qp = o.scale - self.scale - gen.
        // The mantissa for a Num with scale `s` is Q * 10^(qp + s). We pick the
        // final scale now:
        //   fixed mode  -> s = frac (requested)
        //   float mode  -> s = max(0, 13 - integer_digits_of_quotient)
        // Find Q's most-significant index.
        let mut q_msd = 0usize;
        for i in (0..WIDE).rev() {
            if quo_int[i] != 0 {
                q_msd = i;
                break;
            }
        }
        let q_is_zero = quo_int.iter().all(|&x| x == 0);
        // Power of the quotient's most-significant digit: q_msd + qp.
        let qp = o.scale as i16 - self.scale as i16 - gen;
        assemble_quotient(&quo_int, q_msd, q_is_zero, qp, frac,
                          self.neg != o.neg)
    }

    /// Round or truncate to `frac` fractional digits per the rounding switch.
    /// Returns `(result, rounded_up)` where `rounded_up` drives the `^` mark.
    pub fn round_at(&self, frac: u8, round: bool) -> (Num, bool) {
        let mut n = *self;
        if frac >= n.scale {
            let _ = n.rescale(frac);
            return (n, false);
        }
        let (first, _) = n.shr(n.scale - frac);
        n.scale = frac;
        let mut rounded = false;
        if round && first >= 5 {
            rounded = n.inc_mag();
        }
        if n.is_zero() {
            n.neg = false;
        }
        (n, rounded)
    }

    /// Square root of the magnitude to `frac` fractional digits, truncated
    /// (digit-by-digit, firmware §4.5). Sign is ignored: `sqrt(|self|)`.
    pub fn sqrt_at(&self, frac: u8) -> Num {
        // isqrt( self * 10^(2*frac) ), result scale = frac.
        // Place the radicand into a little-endian buffer at index
        // (i - scale) + 2*frac so the integer sqrt lands with `frac` fraction.
        let mut rad = [0u8; WIDE];
        let mut hi = 0usize;
        let base = 2 * frac as i16;
        for i in 0..DIGITS {
            let di = self.digit(i);
            if di == 0 {
                continue;
            }
            let idx = (i as i16 - self.scale as i16) + base;
            if idx < 0 || idx as usize >= WIDE {
                continue;
            }
            let idx = idx as usize;
            rad[idx] = di;
            if idx > hi {
                hi = idx;
            }
        }
        let root = isqrt_digits(&rad, hi);
        let mut n = Num::ZERO;
        for (i, &v) in root.iter().enumerate().take(DIGITS) {
            n.set_digit(i, v);
        }
        n.scale = frac;
        if n.is_zero() {
            n.scale = 0;
        }
        n
    }
}
/// Assemble the final quotient Num from the little-endian integer quotient
/// `quo_int` (MSD at index `q_msd`), quotient power `qp`, and the fixed/float
/// `frac` mode; sign `neg`. `#[inline(never)]` to keep `divide` under a bank.
#[inline(never)]
fn assemble_quotient(quo_int: &[u8; WIDE], q_msd: usize, q_is_zero: bool,
                     qp: i16, frac: Option<u8>, neg: bool) -> Option<Num> {
    let msd_power = q_msd as i16 + qp;
    let int_digits = (msd_power + 1).max(0);
    let final_frac: u8 = match frac {
        Some(n) => n,
        None => (FLOAT_SIG as i16 - int_digits).max(0) as u8,
    };
    if final_frac as usize > DIGITS {
        return None;
    }
    let shift_pow = qp + final_frac as i16;
    let mut quo = Num::ZERO;
    for j in 0..DIGITS {
        let qi = j as i16 - shift_pow;
        if qi < 0 || qi as usize >= WIDE {
            continue;
        }
        quo.set_digit(j, quo_int[qi as usize]);
    }
    if !q_is_zero {
        let top_j = q_msd as i16 + shift_pow;
        if top_j >= DIGITS as i16 {
            return None;
        }
    }
    quo.scale = final_frac;
    quo.neg = neg;
    if quo.is_zero() {
        quo.neg = false;
        quo.scale = 0;
    }
    Some(quo)
}

/// Build the little-endian dividend (scaled up by 10^gen) and divisor buffers
/// for `divide`. Returns None on dividend overflow. `#[inline(never)]` to keep
/// `divide` under a ROM bank.
#[inline(never)]
fn build_div_operands(dividend: &Num, divisor: &Num, gen: i16, n_digits: usize,
                      d_digits: usize) -> Option<([u8; WIDE], [u8; WIDE], usize)> {
    let mut divd = [0u8; WIDE];
    for i in 0..n_digits {
        let idx = i + gen.max(0) as usize;
        if idx >= WIDE {
            return None;
        }
        divd[idx] = dividend.digit(i);
    }
    let mut divs = [0u8; WIDE];
    let mut divs_hi = 0usize;
    for (i, cell) in divs.iter_mut().enumerate().take(d_digits) {
        let oi = divisor.digit(i);
        *cell = oi;
        if oi != 0 {
            divs_hi = i;
        }
    }
    Some((divd, divs, divs_hi))
}




/// Restoring long division: for each position from `dividend_hi` down to 0,
/// count how many times the (shifted) divisor fits into the remaining dividend,
/// subtracting as it goes, and record the count as the quotient digit. Returns
/// the little-endian integer quotient. Kept `#[inline(never)]` so this nested
/// loop stays a standalone function (it is the bulk of `divide`'s compiled
/// size; inlined it pushed `divide` past a ROM bank).
#[inline(never)]
fn restoring_divide(divd: &mut [u8; WIDE], divs: &[u8; WIDE], divs_hi: usize,
                    dividend_hi: usize) -> [u8; WIDE] {
    let mut quo_int = [0u8; WIDE];
    let mut pos = dividend_hi;
    loop {
        let mut count = 0u8;
        while count < 10 && ge_shifted(divd, divs, divs_hi, pos) {
            sub_shifted(divd, divs, divs_hi, pos);
            count += 1;
        }
        if pos < WIDE {
            quo_int[pos] = count;
        }
        if pos == 0 {
            break;
        }
        pos -= 1;
    }
    quo_int
}

/// Compare `rem >= (div << s)` (little-endian decimal, `div_hi` = divisor top).
fn ge_shifted(rem: &[u8; WIDE], div: &[u8; WIDE], div_hi: usize, s: usize) -> bool {
    let top = (div_hi + s).min(WIDE - 1);
    // any remainder digit above `top` makes rem strictly greater
    let mut i = WIDE;
    while i > top + 1 {
        i -= 1;
        if rem[i] != 0 {
            return true;
        }
    }
    let mut k = top as isize;
    while k >= 0 {
        let dk = k as usize;
        let d = if dk >= s && dk - s <= div_hi {
            div[dk - s]
        } else {
            0
        };
        if rem[dk] != d {
            return rem[dk] > d;
        }
        k -= 1;
    }
    true
}

/// `rem -= (div << s)`; caller guarantees `rem >= div<<s`.
fn sub_shifted(rem: &mut [u8; WIDE], div: &[u8; WIDE], div_hi: usize, s: usize) {
    let mut borrow = 0u8;
    for k in 0..WIDE {
        let d = if k >= s && k - s <= div_hi {
            div[k - s]
        } else {
            0
        };
        let sub = d + borrow;
        if rem[k] >= sub {
            rem[k] -= sub;
            borrow = 0;
        } else {
            rem[k] = rem[k] + 10 - sub;
            borrow = 1;
        }
    }
}

/// Integer square root of a big little-endian decimal number `rad[0..=hi]`.
/// Returns the root as a little-endian digit array (firmware §4.5 method:
/// bring down two digits per step, find the largest `x` with
/// `(20*root + x)*x <= remainder`, subtract, append `x`). All `u8` arithmetic.
#[inline(never)]
fn isqrt_digits(rad: &[u8; WIDE], hi: usize) -> [u8; DIGITS] {
    let num_pairs = hi / 2 + 1;
    let mut rem = [0u8; WIDE]; // remainder, little-endian
    let mut root = [0u8; DIGITS]; // little-endian
    let mut root_len = 0usize;

    for p in (0..num_pairs).rev() {
        // remainder = remainder * 100 + next pair
        shift_up(&mut rem, 2);
        let lo = 2 * p;
        let d_hi = if lo < hi { rad[lo + 1] } else { 0 };
        let d_lo = rad[lo];
        add_small(&mut rem, (d_hi * 10 + d_lo) as u16);

        // largest x in 1..=9 with (20*root + x)*x <= remainder
        let mut x = 0u8;
        let mut best = [0u8; WIDE];
        for cand in 1..=9u8 {
            let trial = trial_product(&root, root_len, cand);
            if le_arr(&trial, &rem) {
                x = cand;
                best = trial;
            } else {
                break;
            }
        }
        if x > 0 {
            sub_arr(&mut rem, &best);
        }
        // root = root * 10 + x
        shift_root_up(&mut root, &mut root_len);
        root[0] = x;
    }
    root
}

/// Shift a little-endian decimal buffer up by `n` decimal places (×10^n).
fn shift_up(buf: &mut [u8; WIDE], n: usize) {
    if n == 0 {
        return;
    }
    let mut i = WIDE;
    while i > 0 {
        i -= 1;
        buf[i] = if i >= n { buf[i - n] } else { 0 };
    }
}

/// Shift the little-endian root up by one place (×10), tracking its length.
fn shift_root_up(root: &mut [u8; DIGITS], len: &mut usize) {
    let mut i = DIGITS;
    while i > 1 {
        i -= 1;
        root[i] = root[i - 1];
    }
    root[0] = 0;
    if *len < DIGITS {
        *len += 1;
    }
}

/// Add a small (`< 100`) integer into a little-endian decimal accumulator.
///
/// `v` is a `u16` (not `u32`): both call sites pass a value below 100, and the
/// loop only ever shrinks it, so the whole computation fits in 16 bits. On the
/// 4004 a `u32` here forced `__udivsi3`/`__mulsi3` libcalls whose i32 operands
/// need more double-pair registers than exist, so register allocation ran out;
/// `u16` lowers to the `hi3` helpers and allocates. The integer values are
/// identical, so the host tape stays exact (verified 42/42).
fn add_small(buf: &mut [u8; WIDE], mut v: u16) {
    let mut i = 0usize;
    while v > 0 && i < WIDE {
        let s = buf[i] as u16 + v % 10;
        buf[i] = (s % 10) as u8;
        v = v / 10 + s / 10;
        i += 1;
    }
}

/// `(20*root + cand) * cand` as a little-endian decimal buffer.
#[inline(never)]
fn trial_product(root: &[u8; DIGITS], root_len: usize, cand: u8) -> [u8; WIDE] {
    // base = 2 * root  (little-endian)
    let mut base = [0u8; WIDE];
    let mut carry = 0u8;
    for i in 0..root_len {
        let v = root[i] * 2 + carry;
        base[i] = v % 10;
        carry = v / 10;
    }
    if carry > 0 && root_len < WIDE {
        base[root_len] = carry;
    }
    // base = base * 10 + cand   (i.e. 20*root + cand)
    shift_up(&mut base, 1);
    add_small(&mut base, cand as u16);
    // out = base * cand
    let mut out = [0u8; WIDE];
    let mut c = 0u8;
    for i in 0..WIDE {
        let v = base[i] * cand + c;
        out[i] = v % 10;
        c = v / 10;
    }
    out
}

/// `a <= b` for little-endian decimal buffers.
fn le_arr(a: &[u8; WIDE], b: &[u8; WIDE]) -> bool {
    let mut i = WIDE as isize;
    while i > 0 {
        i -= 1;
        let k = i as usize;
        if a[k] != b[k] {
            return a[k] < b[k];
        }
    }
    true
}

/// `a -= b`; caller guarantees `a >= b`.
fn sub_arr(a: &mut [u8; WIDE], b: &[u8; WIDE]) {
    let mut borrow = 0u8;
    for i in 0..WIDE {
        let s = b[i] + borrow;
        if a[i] >= s {
            a[i] -= s;
            borrow = 0;
        } else {
            a[i] = a[i] + 10 - s;
            borrow = 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(digits: &[u8], scale: u8, neg: bool) -> Num {
        // digits given most significant first
        let mut n = Num::ZERO;
        for (i, &x) in digits.iter().rev().enumerate() {
            n.set_digit(i, x);
        }
        n.scale = scale;
        n.neg = neg;
        n
    }

    /// Low `k` digits, least-significant first, for slice comparisons.
    fn low_digits(n: &Num, k: usize) -> [u8; DIGITS] {
        let mut out = [0u8; DIGITS];
        for (i, slot) in out.iter_mut().enumerate().take(k) {
            *slot = n.digit(i);
        }
        out
    }

    fn render(n: &Num) -> (bool, u8, [u8; DIGITS]) {
        (n.neg, n.scale, low_digits(n, DIGITS))
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

    #[test]
    fn multiply_exact_natural_scale() {
        // 12.3 * 4.56 = 56.088
        let a = num(&[1, 2, 3], 1, false);
        let b = num(&[4, 5, 6], 2, false);
        let r = a.mul(&b).unwrap();
        assert_eq!(r, num(&[5, 6, 0, 8, 8], 3, false));
        // 361.52 * 120 = 43382.40  (scale 2)
        let a = num(&[3, 6, 1, 5, 2], 2, false);
        let b = num(&[1, 2, 0], 0, false);
        let r = a.mul(&b).unwrap();
        assert_eq!(r, num(&[4, 3, 3, 8, 2, 4, 0], 2, false));
    }

    #[test]
    fn multiply_sign() {
        let a = num(&[2], 0, true);
        let b = num(&[3], 0, false);
        assert!(a.mul(&b).unwrap().neg);
        let c = num(&[2], 0, true);
        let d = num(&[3], 0, true);
        assert!(!c.mul(&d).unwrap().neg);
    }

    #[test]
    fn float_division_thirteen_sig() {
        // 40 / 6 = 6.666666666666 (1 int + 12 frac = 13 sig), truncated
        let a = num(&[4, 0], 0, false);
        let b = num(&[6], 0, false);
        let r = a.div_float(&b).unwrap();
        assert_eq!(r.int_digits(), 1);
        assert_eq!(r.scale, 12);
        assert_eq!(
            &low_digits(&r, 13)[..13],
            &[6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6]
        );
    }

    #[test]
    fn float_division_leading_zero() {
        // 2 / 3 = 0.6666666666666 (13 sixes)
        let a = num(&[2], 0, false);
        let b = num(&[3], 0, false);
        let r = a.div_float(&b).unwrap();
        assert_eq!(r.scale, 13);
        assert_eq!(r.int_digits(), 0);
        assert_eq!(
            &low_digits(&r, 13)[..13],
            &[6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6]
        );
    }

    #[test]
    fn fixed_division_rounds_and_truncates() {
        // 40 / 6 at DP=2: truncate -> 6.66, round -> 6.67 (rounded mark)
        let a = num(&[4, 0], 0, false);
        let b = num(&[6], 0, false);
        let q = a.div_fixed(&b, 3).unwrap(); // 3 guard digits: 6.666
        let (t, mark) = q.round_at(2, false);
        assert_eq!(t, num(&[6, 6, 6], 2, false));
        assert!(!mark);
        let (r, mark) = q.round_at(2, true);
        assert_eq!(r, num(&[6, 6, 7], 2, false));
        assert!(mark);
    }

    #[test]
    fn square_root_exact_and_fractional() {
        // sqrt(208) to 6 places = 14.422205 (truncated)
        let a = num(&[2, 0, 8], 0, false);
        let r = a.sqrt_at(6);
        assert_eq!(r.scale, 6);
        assert_eq!(&low_digits(&r, 8)[..8], &[5, 0, 2, 2, 2, 4, 4, 1]); // 14.422205 little-endian
                                                                        // sqrt(152417543.0625) to 9 places = 12345.750000000
        let a = num(&[1, 5, 2, 4, 1, 7, 5, 4, 3, 0, 6, 2, 5], 4, false);
        let r = a.sqrt_at(9);
        let (_neg, scale, _d) = render(&r);
        assert_eq!(scale, 9);
        // value 12345.750000000
        assert_eq!(r.int_digits(), 5);
    }
}
