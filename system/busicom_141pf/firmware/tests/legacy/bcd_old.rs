//! The pre-issue-#18 `bcd.rs` (packed `[u8; 8]` digits, by-value `Num`),
//! kept verbatim as the reference for `tests/bcd_equivalence.rs`.
#![allow(dead_code, unused_imports, unused_unsafe, clippy::all)]

pub mod regops {
    //! Block digit-register primitives (Tier-A size co-design).
    //!
    //! The arithmetic core works on `Num`s whose 16 decimal digits are packed two
    //! per byte (`[u8; 8]`, low nibble = even digit). Doing each digit loop in Rust
    //! (`digit`/`set_digit` with mask+shift, 16 iterations) compiles to a large
    //! per-digit instruction burst on the 4004. These primitives move the hottest
    //! whole-register loops into ONE call each, so the target build emits a single
    //! compact routine (an `SRC` + `RDM`/`WRM`/`INC` loop in libi4004rt assembly)
    //! instead of inlining the unpacking 16 times.
    //!
    //! Two implementations behind the same `extern "C"` ABI:
    //!   * host (`cfg(not(target_os = "none"))`): plain Rust over the packed bytes,
    //!     byte-for-byte identical to the loops it replaces, so host tests stay
    //!     42/42 exact;
    //!   * target (`cfg(target_os = "none")`): the symbols are provided by
    //!     libi4004rt assembly (`llvm-i4004/rt/regops.s`), linked at image time.
    //!
    //! All primitives take raw pointers to the packed 8-byte digit arrays. `DIGITS`
    //! (16) nibbles per register; digit `i` is the low nibble of `d[i>>1]` when `i`
    //! is even, the high nibble when odd. Values are 0..=9 BCD.

    use super::DIGITS;

    // Packed accessor helpers shared by the host impl (and by callers building the
    // pointer-based calls). `d` is the 8-byte packed array.
    #[inline(always)]
    pub fn get_digit(d: &[u8], i: usize) -> u8 {
        let b = d[i >> 1];
        if i & 1 == 0 {
            b & 0x0F
        } else {
            b >> 4
        }
    }

    #[inline(always)]
    pub fn set_digit(d: &mut [u8], i: usize, v: u8) {
        let idx = i >> 1;
        let b = d[idx];
        d[idx] = if i & 1 == 0 {
            (b & 0xF0) | (v & 0x0F)
        } else {
            (b & 0x0F) | (v << 4)
        };
    }

    unsafe fn slice8<'a>(p: *const u8) -> &'a [u8] {
        core::slice::from_raw_parts(p, DIGITS / 2)
    }
    unsafe fn slice8_mut<'a>(p: *mut u8) -> &'a mut [u8] {
        core::slice::from_raw_parts_mut(p, DIGITS / 2)
    }

    // ---------------------------------------------------------------------------
    // Host implementations. Target provides these from libi4004rt assembly.
    // The contracts match the Rust loops in bcd.rs exactly.
    // ---------------------------------------------------------------------------
    #[cfg(not(target_os = "none"))]
    mod host {
        use super::*;

        /// a += b (BCD, 16 digits). Returns 1 if there was NO carry out (fits),
        /// 0 if the magnitude overflowed the register. Mirrors `Num::add_mag`.

        pub extern "C" fn reg_add(a: *mut u8, b: *const u8) -> u8 {
            let a = unsafe { slice8_mut(a) };
            let b = unsafe { slice8(b) };
            let mut carry = 0u8;
            for i in 0..DIGITS {
                let mut s = get_digit(a, i) + get_digit(b, i) + carry;
                carry = 0;
                if s > 9 {
                    s -= 10;
                    carry = 1;
                }
                set_digit(a, i, s);
            }
            (carry == 0) as u8
        }

        /// a -= b (BCD, 16 digits); caller guarantees a >= b. Mirrors `Num::sub_mag`.

        pub extern "C" fn reg_sub(a: *mut u8, b: *const u8) {
            let a = unsafe { slice8_mut(a) };
            let b = unsafe { slice8(b) };
            let mut borrow = 0u8;
            for i in 0..DIGITS {
                let sub = get_digit(b, i) + borrow;
                let di = get_digit(a, i);
                if di >= sub {
                    set_digit(a, i, di - sub);
                    borrow = 0;
                } else {
                    set_digit(a, i, di + 10 - sub);
                    borrow = 1;
                }
            }
        }

        /// Compare magnitudes: returns 2 if a>b, 0 if a<b, 1 if equal (so the caller
        /// maps to Ordering without a signed return). Mirrors `Num::cmp_mag`.

        pub extern "C" fn reg_cmp(a: *const u8, b: *const u8) -> u8 {
            let a = unsafe { slice8(a) };
            let b = unsafe { slice8(b) };
            let mut i = DIGITS;
            while i > 0 {
                i -= 1;
                let (x, y) = (get_digit(a, i), get_digit(b, i));
                if x != y {
                    return if x > y { 2 } else { 0 };
                }
            }
            1
        }

        /// All 16 digits zero? 1 = yes.

        pub extern "C" fn reg_is_zero(a: *const u8) -> u8 {
            let a = unsafe { slice8(a) };
            (a.iter().all(|&x| x == 0)) as u8
        }

        /// dst = src (8 packed bytes).

        pub extern "C" fn reg_copy(dst: *mut u8, src: *const u8) {
            let d = unsafe { slice8_mut(dst) };
            let s = unsafe { slice8(src) };
            d.copy_from_slice(s);
        }

        /// Zero all 16 digits.

        pub extern "C" fn reg_clear(a: *mut u8) {
            let a = unsafe { slice8_mut(a) };
            for x in a.iter_mut() {
                *x = 0;
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Public safe wrappers. Both builds call these; on target they resolve to the
    // libi4004rt assembly symbols, on host to the module above.
    // ---------------------------------------------------------------------------
    #[cfg(target_os = "none")]
    extern "C" {
        fn reg_add(a: *mut u8, b: *const u8) -> u8;
        fn reg_sub(a: *mut u8, b: *const u8);
        fn reg_cmp(a: *const u8, b: *const u8) -> u8;
        fn reg_is_zero(a: *const u8) -> u8;
        fn reg_copy(dst: *mut u8, src: *const u8);
        fn reg_clear(a: *mut u8);
    }

    #[cfg(not(target_os = "none"))]
    use host::{reg_add, reg_clear, reg_cmp, reg_copy, reg_is_zero, reg_sub};

    /// a += b (BCD). Returns true if it fit (no carry out).
    #[inline]
    pub fn add(a: &mut [u8; 8], b: &[u8; 8]) -> bool {
        unsafe { reg_add(a.as_mut_ptr(), b.as_ptr()) != 0 }
    }

    /// a -= b (BCD); caller guarantees a >= b.
    #[inline]
    pub fn sub(a: &mut [u8; 8], b: &[u8; 8]) {
        unsafe { reg_sub(a.as_mut_ptr(), b.as_ptr()) }
    }

    /// Magnitude ordering of a vs b.
    #[inline]
    pub fn cmp(a: &[u8; 8], b: &[u8; 8]) -> core::cmp::Ordering {
        use core::cmp::Ordering::*;
        match unsafe { reg_cmp(a.as_ptr(), b.as_ptr()) } {
            2 => Greater,
            0 => Less,
            _ => Equal,
        }
    }

    #[inline]
    pub fn is_zero(a: &[u8; 8]) -> bool {
        unsafe { reg_is_zero(a.as_ptr()) != 0 }
    }

    #[inline]
    pub fn copy(dst: &mut [u8; 8], src: &[u8; 8]) {
        unsafe { reg_copy(dst.as_mut_ptr(), src.as_ptr()) }
    }

    #[inline]
    pub fn clear(a: &mut [u8; 8]) {
        unsafe { reg_clear(a.as_mut_ptr()) }
    }
}

// Digit-serial BCD numbers, the representation the original keeps in 4002
// registers: 16 decimal digits (one per nibble), least significant first,
// plus a sign and the number of fractional digits.
//
// Every operation works one digit at a time with `u8` carries, so the code
// maps onto 4004 `ADD`/`SUB`/`DAA`-style loops without wider integers. The
// multiply/divide/sqrt helpers use fixed-size `u8` scratch buffers (products
// of two 14-digit numbers need up to 28 digits); none use `u16`+ arithmetic
// for digit values.

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
        regops::is_zero(&self.d)
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
        regops::cmp(&self.d, &other.d)
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
        regops::add(&mut self.d, &o.d)
    }

    /// self.mag -= o.mag, requires self.mag >= o.mag.
    fn sub_mag(&mut self, o: &Num) {
        regops::sub(&mut self.d, &o.d)
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
    /// `#[inline(never)]`: the `set_digit` placement loop is sizeable, and
    /// keeping it out of line keeps `Num::mul` (its caller) under a ROM bank.
    #[inline(never)]
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
        // The digit accumulation loop is extracted to a #[inline(never)] helper
        // so mul stays under a 4004 ROM bank window. The WIDE accumulator is
        // owned here and passed by &mut (no by-value array return -> smaller
        // frame, no sret copy).
        let mut acc = [0u8; WIDE];
        if !mul_accumulate(self, o, &mut acc) {
            return None;
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

        // Fractional-digit count to grind, incl. one guard digit (extracted to
        // a #[inline(never)] helper so `divide` stays under a 4004 ROM bank).
        let gen = div_gen(frac, n_digits, d_digits, self.scale, o.scale);

        // Long division into `quo_int` (little-endian integer quotient). The
        // dividend/divisor WIDE scratch (divd, divs) is owned INSIDE long_divide,
        // so `divide`'s own frame carries only this one WIDE buffer (not three)
        // -- that is what keeps `divide` under a 256-char CM-RAM frame bank.
        let dividend_hi = n_digits + gen.max(0) as usize; // one past top is fine
        let mut quo_int = [0u8; WIDE];
        if !long_divide(self, o, gen, n_digits, d_digits, dividend_hi, &mut quo_int) {
            return None;
        }

        // Q value = quo_int (little-endian integer). The quotient value is
        //   Q * 10^qp,  qp = o.scale - self.scale - gen.
        // assemble_quotient finds Q's most-significant index itself (the scan
        // lives there so `divide` stays under a ROM bank).
        let qp = o.scale as i16 - self.scale as i16 - gen;
        assemble_quotient(&quo_int, qp, frac, self.neg != o.neg)
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
/// Number of fractional digits `divide` grinds (incl. one guard digit): `frac`
/// in fixed mode, else enough to cover FLOAT_SIG significant digits regardless
/// of the quotient's integer size. `#[inline(never)]` to keep `divide` under a
/// 4004 ROM bank window.
#[inline(never)]
fn div_gen(frac: Option<u8>, n_digits: usize, d_digits: usize, self_scale: u8, o_scale: u8) -> i16 {
    let num_exp = n_digits as i16 - 1 - self_scale as i16;
    let den_exp = d_digits as i16 - 1 - o_scale as i16;
    let lead_hi = num_exp - den_exp + 1;
    let gen_frac: u8 = match frac {
        // Corrected 2026-10-22 together with `bcd::Num::div` (this reference
        // carried the same shortfall): grind sb - sa more digits when the
        // divisor has more fractional digits than the dividend.
        Some(n) => n + (o_scale as i16 - self_scale as i16).max(0) as u8,
        None => {
            let depth = FLOAT_SIG as i16 - lead_hi.min(0); // >= FLOAT_SIG
            depth.max(FLOAT_SIG as i16) as u8
        }
    };
    gen_frac as i16 + 1
}
/// Assemble the final quotient Num from the little-endian integer quotient
/// `quo_int` (MSD at index `q_msd`), quotient power `qp`, and the fixed/float
/// `frac` mode; sign `neg`. `#[inline(never)]` to keep `divide` under a bank.
#[inline(never)]
fn assemble_quotient(quo_int: &[u8; WIDE], qp: i16, frac: Option<u8>, neg: bool) -> Option<Num> {
    // Most-significant index of the little-endian integer quotient (folded in
    // from `divide` to keep that function under a ROM bank).
    let mut q_msd = 0usize;
    for i in (0..WIDE).rev() {
        if quo_int[i] != 0 {
            q_msd = i;
            break;
        }
    }
    let q_is_zero = quo_int.iter().all(|&x| x == 0);
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
    place_quotient_digits(&mut quo, quo_int, shift_pow);
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

/// Place the little-endian integer quotient `quo_int` into `quo`'s packed
/// digits, digit `j` taken from buffer index `j - shift_pow`. `#[inline(never)]`
/// so the packed-`set_digit` loop stays out of `assemble_quotient` (keeping it
/// under a 4004 ROM bank).
#[inline(never)]
fn place_quotient_digits(quo: &mut Num, quo_int: &[u8; WIDE], shift_pow: i16) {
    for j in 0..DIGITS {
        let qi = j as i16 - shift_pow;
        if qi < 0 || qi as usize >= WIDE {
            continue;
        }
        quo.set_digit(j, quo_int[qi as usize]);
    }
}

/// Drive the whole integer long division into `quo_int`, owning the dividend
/// and divisor WIDE scratch buffers INSIDE this frame so `Num::divide` carries
/// only `quo_int` (one WIDE buffer) rather than three. That is what keeps
/// `divide`'s per-function frame under the 256-char CM-RAM bank. Returns false
/// on dividend overflow (propagated as `None` by the caller). `#[inline(never)]`.
#[inline(never)]
fn long_divide(
    dividend: &Num,
    divisor: &Num,
    gen: i16,
    n_digits: usize,
    d_digits: usize,
    dividend_hi: usize,
    quo_int: &mut [u8; WIDE],
) -> bool {
    let mut divd = [0u8; WIDE];
    let mut divs = [0u8; WIDE];
    let divs_hi = match build_div_operands(
        dividend, divisor, gen, n_digits, d_digits, &mut divd, &mut divs,
    ) {
        Some(x) => x,
        None => return false,
    };
    restoring_divide(&mut divd, &divs, divs_hi, dividend_hi, quo_int);
    true
}

/// Build the little-endian dividend (scaled up by 10^gen) and divisor buffers
/// for `divide`, writing into caller-provided scratch so neither this function
/// nor `divide` has to hold a 2×WIDE return buffer on its tiny 4004 frame.
/// Returns `Some(divs_hi)` (top nonzero divisor index) or `None` on dividend
/// overflow. `#[inline(never)]` to keep `divide` under a ROM bank.
#[inline(never)]
fn build_div_operands(
    dividend: &Num,
    divisor: &Num,
    gen: i16,
    n_digits: usize,
    d_digits: usize,
    divd: &mut [u8; WIDE],
    divs: &mut [u8; WIDE],
) -> Option<usize> {
    for i in 0..n_digits {
        let idx = i + gen.max(0) as usize;
        if idx >= WIDE {
            return None;
        }
        divd[idx] = dividend.digit(i);
    }
    let mut divs_hi = 0usize;
    for (i, cell) in divs.iter_mut().enumerate().take(d_digits) {
        let oi = divisor.digit(i);
        *cell = oi;
        if oi != 0 {
            divs_hi = i;
        }
    }
    Some(divs_hi)
}

/// Schoolbook multiply accumulation: acc = sum over digits i of self.digit(i) *
/// o shifted left i. Writes into caller-provided `acc` (out-param, no by-value
/// WIDE return) and returns `false` on register overflow. `#[inline(never)]` so
/// mul stays under a 4004 ROM bank window.
#[inline(never)]
fn mul_accumulate(a: &Num, o: &Num, acc: &mut [u8; WIDE]) -> bool {
    for i in 0..DIGITS {
        let di = a.digit(i);
        if di == 0 {
            continue;
        }
        if !mul_add_row(acc, di, o, i) {
            return false;
        }
    }
    true
}
/// One schoolbook multiply row: acc += di * o, shifted left by `i` digits
/// (little-endian), with carry propagation into the high tail. Returns false on
/// register overflow. `#[inline(never)]` to keep `Num::mul` under a ROM bank.
#[inline(never)]
fn mul_add_row(acc: &mut [u8; WIDE], di: u8, o: &Num, i: usize) -> bool {
    let mut carry = 0u8;
    for j in 0..DIGITS {
        let pos = i + j;
        let oj = o.digit(j);
        if pos >= WIDE {
            if oj != 0 {
                return false;
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
            return false;
        }
        let p = acc[pos] + carry;
        acc[pos] = p % 10;
        carry = p / 10;
        pos += 1;
    }
    true
}

/// Restoring long division: for each position from `dividend_hi` down to 0,
/// count how many times the (shifted) divisor fits into the remaining dividend,
/// subtracting as it goes, and record the count as the quotient digit. Writes
/// the little-endian integer quotient into caller-provided `quo_int` (out-param
/// rather than a by-value return so neither this function nor `divide` carries
/// an extra WIDE buffer on its 4004 frame). Kept `#[inline(never)]` so this
/// nested loop stays a standalone function (it is the bulk of `divide`'s
/// compiled size; inlined it pushed `divide` past a ROM bank).
#[inline(never)]
fn restoring_divide(
    divd: &mut [u8; WIDE],
    divs: &[u8; WIDE],
    divs_hi: usize,
    dividend_hi: usize,
    quo_int: &mut [u8; WIDE],
) {
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

        // Find this step's root digit x and subtract (20*root+x)*x from rem.
        // The candidate scan + subtract (and its WIDE `trial` scratch) live in
        // a #[inline(never)] helper so isqrt_digits' own compiled size stays
        // under a 4004 ROM bank and it carries no WIDE trial buffer.
        let x = isqrt_next_digit(&mut rem, &root, root_len);
        // root = root * 10 + x
        shift_root_up(&mut root, &mut root_len);
        root[0] = x;
    }
    root
}

/// One square-root step: the largest `x` in 1..=9 with
/// `(20*root + x)*x <= rem`, subtracting that product from `rem`. Returns `x`.
/// Owns the WIDE `trial` scratch here (one buffer, recomputed for the chosen
/// candidate) so `isqrt_digits` neither carries it nor inlines this bulk --
/// keeping isqrt_digits under a 4004 ROM bank. `#[inline(never)]`.
#[inline(never)]
fn isqrt_next_digit(rem: &mut [u8; WIDE], root: &[u8; DIGITS], root_len: usize) -> u8 {
    let mut x = 0u8;
    let mut trial = [0u8; WIDE];
    for cand in 1..=9u8 {
        trial_product(root, root_len, cand, &mut trial);
        if le_arr(&trial, rem) {
            x = cand;
        } else {
            break;
        }
    }
    if x > 0 {
        trial_product(root, root_len, x, &mut trial);
        sub_arr(rem, &trial);
    }
    x
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

/// `(20*root + cand) * cand` as a little-endian decimal buffer, written into
/// caller-provided `out` (out-param rather than a by-value WIDE return, so
/// `isqrt_digits` does not carry an extra buffer per candidate on its frame).
#[inline(never)]
fn trial_product(root: &[u8; DIGITS], root_len: usize, cand: u8, out: &mut [u8; WIDE]) {
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
    let mut c = 0u8;
    for i in 0..WIDE {
        let v = base[i] * cand + c;
        out[i] = v % 10;
        c = v / 10;
    }
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
