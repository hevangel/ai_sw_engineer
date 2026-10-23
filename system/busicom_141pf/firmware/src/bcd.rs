//! Digit-serial BCD numbers kept where the original keeps them: in 4002 RAM
//! registers, one decimal digit per 4-bit character (issue #18, B1/B2).
//!
//! A [`Num`] is a one-byte **handle**: the `SRC` address of character 0 of a
//! register. Digits 0..15 (least significant first) are the register's 16 main
//! characters; the sign is status character S0 and the scale (number of
//! fractional digits) is the byte S2:S1. Every operation works in place on
//! registers -- there is no `Num` value to copy, no packed byte array, and no
//! integer wider than `u8`.
//!
//! Long intermediates (the 32-digit product, the 34-digit dividend, the
//! square-root remainder) live in consecutive scratch characters: the `SRC`
//! address is linear across registers, so digit `k` of a buffer is
//! `base + k`.
//!
//! The digit loops are the [`prim`] leaves: hand-written MCS-4 routines on the
//! target (`asm/num.s`: `ADM`/`DAA` decimal add, `TCS`/`SBM` decimal subtract,
//! `SRC`/`RDM`/`WRM` walks) and bit-exact Rust twins on the host. Everything
//! above them -- scales, signs, the division and square-root bookkeeping --
//! is this one Rust source for both. The algorithms reproduce the previous
//! `bcd.rs` exactly (42/42 host tape, `tests/bcd_equivalence.rs`):
//! multiplication by repeated BCD addition, restoring division, and square
//! root by subtracting successive odd terms (`20r+1, 20r+3, ...`), which
//! yields the same digit as the largest-`x` trial.

use crate::ram::{rd, srd, swr, wr};

/// Digits per register (one 4002 register holds 16 main characters).
pub const DIGITS: u8 = 16;
/// Printable / storable digits (manual: "14 digits plus decimal point and sign").
pub const CAPACITY: u8 = 14;
/// Floating multiply/divide significant-digit budget (recovered-ROM behaviour).
pub const FLOAT_SIG: u8 = 13;
/// Width of the division dividend window, in digits.
const WIDE: u8 = 34;
/// `frac` value asking [`Num::div`] for the floating 13-significant-digit result.
pub const FLOAT: u8 = 0xFF;

/// A handle to a 4002 register holding a BCD number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Num(pub u8);

// --- RAM map (CM-RAM bank 0, see platform/memory_map.md) -------------------
/// Working / entry register `WR`.
pub const WR: Num = Num(0x00);
/// Dividend / multiplicand register `DR`.
pub const DR: Num = Num(0x10);
/// Constant register `CR`.
pub const CR: Num = Num(0x20);
/// Subtotal register `SR`.
pub const SR: Num = Num(0x30);
/// Main total register `TR`.
pub const TR: Num = Num(0x40);
/// Memory register `MR`.
pub const MR: Num = Num(0x50);
/// Result scratch registers (a computed value before it is committed).
pub const T1: Num = Num(0x60);
pub const T2: Num = Num(0x70);
/// Scratch area 0x80..0xBF (four registers), live inside one operation only.
const XA: Num = Num(0x80);
const XB: Num = Num(0x90);
/// Product buffer (32 digits).
const MULBUF: u8 = 0x80;
/// Division remainder (17 digits: below 10 * divisor): it ends exactly at
/// 0xB0, the boundary the division primitive stops at.
pub(crate) const DIVREM: u8 = 0xB0 - 17;
/// Square-root remainder and odd term `20r + 2x + 1` (20 digits each: below
/// 2*10^18 after `* 100 + pair`).
pub(crate) const SQ_REM: u8 = 0x80;
pub(crate) const SQ_S: u8 = 0x94;
const SQ_LEN: u8 = 20;
/// The 16-digit quotient window (division) / the root being built (sqrt).
const WIN: Num = Num(0xB0);

/// The digit-loop leaves: `asm/num.s` on the target, Rust twins on the host.
/// Buffer lengths are passed negated (`ISZ` counts up to zero).
pub mod prim {
    #[cfg(target_os = "none")]
    mod asm {
        extern "C" {
            #[link_name = "__leaf_n_clear"]
            pub fn __n_clear(a: u8);
            #[link_name = "__leaf_n_copy"]
            pub fn __n_copy(dst: u8, src: u8);
            #[link_name = "__leaf_n_cp"]
            pub fn __n_cp(packed: u8);
            #[link_name = "__leaf_n_cpyd"]
            pub fn __n_cpyd(dst: u8, src: u8);
            #[link_name = "__leaf_n_digits"]
            pub fn __n_digits(a: u8) -> u8;
            #[link_name = "__leaf_n_width"]
            pub fn __n_width(a: u8) -> u8;
            #[link_name = "__leaf_n_cmp"]
            pub fn __n_cmp(a: u8, b: u8) -> u8;
            #[link_name = "__leaf_n_add16"]
            pub fn __n_add16(a: u8, b: u8) -> u8;
            #[link_name = "__leaf_n_sub16"]
            pub fn __n_sub16(a: u8, b: u8);
            #[link_name = "__leaf_n_inc"]
            pub fn __n_inc(a: u8) -> u8;
            #[link_name = "__leaf_n_shl"]
            pub fn __n_shl(a: u8, k: u8) -> u8;
            #[link_name = "__leaf_n_shr"]
            pub fn __n_shr(a: u8, k: u8) -> u8;
            #[link_name = "__leaf_b_clear"]
            pub fn __b_clear(buf: u8, nlen: u8);
            #[link_name = "__leaf_b_up"]
            pub fn __b_up(buf: u8, nlen: u8);
            #[link_name = "__leaf_b_addsh"]
            pub fn __b_addsh(at: u8, o: u8);
            #[link_name = "__leaf_b_divdig"]
            pub fn __b_divdig(div: u8) -> u8;
            #[link_name = "__leaf_b_sqdig"]
            pub fn __b_sqdig() -> u8;
        }
    }
    #[cfg(target_os = "none")]
    use asm::*;

    /// Whole-operation entry routines (asm/num.s, after the leaves). They call
    /// leaves and clobber P0-P5.
    #[cfg(target_os = "none")]
    pub mod entry {
        extern "C" {
            pub fn __n_div(dst_frac: u8, a_fl: u8, b: u8) -> u8;
            pub fn __n_mul(dst: u8, a: u8, b: u8) -> u8;
            pub fn __n_add(dst_neg: u8, a: u8, b: u8) -> u8;
            pub fn __n_round(dst: u8, a: u8) -> u8;
            pub fn __n_sqrt(dst: u8, a: u8, frac: u8);
            pub fn __n_compute() -> u8;
        }
    }
    #[cfg(not(target_os = "none"))]
    use host::*;

    macro_rules! leaf {
        ($(#[$m:meta])* $name:ident => $sym:ident ($($a:ident),*) $(-> $r:ty)?) => {
            $(#[$m])*
            #[inline(always)]
            pub fn $name($($a: u8),*) $(-> $r)? {
                #[allow(unused_unsafe)]
                unsafe { $sym($($a),*) }
            }
        };
    }
    leaf!(/// Digits, sign and scale = 0.
          clear => __n_clear(a));
    leaf!(/// Digits, sign and scale.
          copy => __n_copy(dst, src));
    leaf!(/// [`copy`] with `dst | src >> 4` in one byte.
          cp => __n_cp(packed));
    leaf!(/// Digits only (`src` may be a register-aligned buffer).
          cpyd => __n_cpyd(dst, src));
    leaf!(/// Significant digits, 0..=16.
          digits => __n_digits(a) -> u8);
    leaf!(/// Digits printed: `max(digits, scale + 1)`.
          width => __n_width(a) -> u8);
    leaf!(/// Magnitudes: 0 less, 1 equal, 2 greater.
          cmp => __n_cmp(a, b) -> u8);
    leaf!(/// `a += b` (16 digits); the carry out.
          add16 => __n_add16(a, b) -> u8);
    leaf!(/// `a -= b` (16 digits, `a >= b`).
          sub16 => __n_sub16(a, b));
    leaf!(/// `a += 1`; the carry out of digit 15.
          inc => __n_inc(a) -> u8);
    leaf!(/// `a *= 10^k`; 0 (unchanged) if digits would be lost.
          shl => __n_shl(a, k) -> u8);
    leaf!(/// `a /= 10^k` truncating; the most significant dropped digit.
          shr => __n_shr(a, k) -> u8);
    leaf!(/// Zero `-nlen` characters from `buf`.
          b_clear => __b_clear(buf, nlen));
    leaf!(/// Shift a `-nlen`-digit buffer up one place (top dropped).
          b_up => __b_up(buf, nlen));
    leaf!(/// Add register `o` into the buffer at `at`, carry rippling up.
          b_addsh => __b_addsh(at, o));
    leaf!(/// One long-division digit: the count of `div` in the remainder at
          /// `DIVREM` (see `asm/num.s`).
          b_divdig => __b_divdig(div) -> u8);
    leaf!(/// One square-root digit (see `asm/num.s`).
          b_sqdig => __b_sqdig() -> u8);

    /// Bit-exact host twins of `asm/num.s` (the chips execution test,
    /// llvm-i4004/scripts/exec_num.py, compares the two).
    #[cfg(not(target_os = "none"))]
    #[allow(clippy::missing_safety_doc)]
    pub mod host {
        use crate::ram::{rd, srd, swr, wr};

        fn len_of(nlen: u8) -> u16 {
            if nlen == 0 {
                256
            } else {
                0x100 - nlen as u16
            }
        }

        pub unsafe fn __n_clear(a: u8) {
            for i in 0..16 {
                wr(a + i, 0);
            }
            for k in 0..3 {
                swr(a | k, 0);
            }
        }
        pub unsafe fn __n_cp(packed: u8) {
            __n_copy(packed & 0xF0, packed << 4);
        }
        pub unsafe fn __n_copy(dst: u8, src: u8) {
            for k in 0..3 {
                swr(dst | k, srd(src | k));
            }
            __n_cpyd(dst, src);
        }
        pub unsafe fn __n_cpyd(dst: u8, src: u8) {
            for i in 0..16 {
                wr(dst + i, rd(src + i));
            }
        }
        pub unsafe fn __n_digits(a: u8) -> u8 {
            let mut n = 16;
            while n > 0 && rd(a + n - 1) == 0 {
                n -= 1;
            }
            n
        }
        pub unsafe fn __n_width(a: u8) -> u8 {
            let len = __n_digits(a);
            let s = srd(a | 1) | (srd(a | 2) << 4);
            if len > s {
                len
            } else {
                s.wrapping_add(1)
            }
        }
        pub unsafe fn __n_cmp(a: u8, b: u8) -> u8 {
            for i in (0..16).rev() {
                let (x, y) = (rd(a + i), rd(b + i));
                if x != y {
                    return if x > y { 2 } else { 0 };
                }
            }
            1
        }
        pub unsafe fn __n_add16(a: u8, b: u8) -> u8 {
            let mut c = 0;
            for i in 0..16 {
                let s = rd(a + i) + rd(b + i) + c;
                c = (s > 9) as u8;
                wr(a + i, if s > 9 { s - 10 } else { s });
            }
            c
        }
        pub unsafe fn __n_sub16(a: u8, b: u8) {
            let mut borrow = 0;
            for i in 0..16 {
                let sub = rd(b + i) + borrow;
                let x = rd(a + i);
                borrow = (x < sub) as u8;
                wr(a + i, if x >= sub { x - sub } else { x + 10 - sub });
            }
        }
        pub unsafe fn __n_inc(a: u8) -> u8 {
            for i in 0..16 {
                let x = rd(a + i);
                if x < 9 {
                    wr(a + i, x + 1);
                    return 0;
                }
                wr(a + i, 0);
            }
            1
        }
        pub unsafe fn __n_shl(a: u8, k: u8) -> u8 {
            if k > 16 {
                return 0;
            }
            let n = __n_digits(a);
            if n == 0 {
                return 1;
            }
            if n + k > 16 {
                return 0;
            }
            for i in (0..16u8).rev() {
                wr(a + i, if i >= k { rd(a + i - k) } else { 0 });
            }
            1
        }
        pub unsafe fn __n_shr(a: u8, k: u8) -> u8 {
            if k == 0 {
                return 0;
            }
            if k > 16 {
                for i in 0..16 {
                    wr(a + i, 0);
                }
                return 0;
            }
            let first = rd(a + k - 1);
            for i in 0..16u8 {
                wr(a + i, if i + k < 16 { rd(a + i + k) } else { 0 });
            }
            first
        }
        pub unsafe fn __b_clear(buf: u8, nlen: u8) {
            for i in 0..len_of(nlen) {
                wr(buf.wrapping_add(i as u8), 0);
            }
        }
        pub unsafe fn __b_up(buf: u8, nlen: u8) {
            let mut carry = 0;
            for i in 0..len_of(nlen) {
                let p = buf.wrapping_add(i as u8);
                let t = rd(p);
                wr(p, carry);
                carry = t;
            }
        }
        pub unsafe fn __b_addsh(at: u8, o: u8) {
            let mut c = 0;
            let mut p = at;
            for i in 0..16 {
                let s = rd(p) + rd(o + i) + c;
                c = (s > 9) as u8;
                wr(p, if s > 9 { s - 10 } else { s });
                p = p.wrapping_add(1);
            }
            while c != 0 {
                let s = rd(p) + 1;
                c = (s > 9) as u8;
                wr(p, if s > 9 { s - 10 } else { s });
                p = p.wrapping_add(1);
            }
        }
        /// `rem` window vs `div` (digits 0..15, then zeros), from `rem` up to
        /// (excluding) 0xB0. `write`: subtract; else only compare. Returns
        /// "no borrow" (rem >= div).
        fn divpass(div: u8, rem: u8, write: bool) -> bool {
            let mut borrow = 0;
            let mut p = rem;
            let mut j = 0u8;
            while p < 0xB0 {
                let d = if j < 16 { rd(div + j) } else { 0 };
                let sub = d + borrow;
                let x = rd(p);
                borrow = (x < sub) as u8;
                if write {
                    wr(p, if x >= sub { x - sub } else { x + 10 - sub });
                }
                p += 1;
                j += 1;
            }
            borrow == 0
        }
        /// The remainder is the 17 digits at `DIVREM` (below 10 * div, so the
        /// count is a digit).
        pub unsafe fn __b_divdig(div: u8) -> u8 {
            let mut count = 0;
            while divpass(div, super::super::DIVREM, false) {
                divpass(div, super::super::DIVREM, true);
                count += 1;
            }
            count
        }
        /// The remainder and odd term are the 20-digit buffers at `SQ_REM` /
        /// `SQ_S`.
        pub unsafe fn __b_sqdig() -> u8 {
            let (rem, s) = (super::super::SQ_REM, super::super::SQ_S);
            let mut x = 0;
            loop {
                // compare
                let mut borrow = 0;
                for i in 0..20 {
                    let sub = rd(s + i) + borrow;
                    borrow = (rd(rem + i) < sub) as u8;
                }
                if borrow != 0 {
                    return x;
                }
                let mut borrow = 0;
                for i in 0..20 {
                    let sub = rd(s + i) + borrow;
                    let v = rd(rem + i);
                    borrow = (v < sub) as u8;
                    wr(rem + i, if v >= sub { v - sub } else { v + 10 - sub });
                }
                let mut add = 2;
                let mut p = s;
                while add != 0 {
                    let v = rd(p) + add;
                    add = (v > 9) as u8;
                    wr(p, if v > 9 { v - 10 } else { v });
                    p += 1;
                }
                x += 1;
            }
        }
    }
}

impl Num {
    /// Digit `i` (0 = least significant), `i < 16`.
    #[inline(always)]
    pub fn digit(self, i: u8) -> u8 {
        rd(self.0 + i)
    }
    #[inline(always)]
    pub fn set_digit(self, i: u8, v: u8) {
        wr(self.0 + i, v)
    }
    /// Address of status character `k`. A handle is register-aligned, but the
    /// compiler cannot know that of a run-time handle; masking the character
    /// nibble makes `k` a constant, so the access is a single `RDk`/`WRk`
    /// instead of a switch over all four.
    #[inline(always)]
    fn status(self, k: u8) -> u8 {
        (self.0 & 0xF0) | k
    }
    #[inline(always)]
    pub fn is_neg(self) -> bool {
        srd(self.status(0)) != 0
    }
    #[inline(always)]
    pub fn set_neg(self, n: bool) {
        swr(self.status(0), n as u8)
    }
    /// Number of fractional digits (a byte: a product's natural scale can
    /// reach 26 before it is rounded).
    #[inline(always)]
    pub fn scale(self) -> u8 {
        srd(self.status(1)) | (srd(self.status(2)) << 4)
    }
    #[inline(always)]
    pub fn set_scale(self, s: u8) {
        swr(self.status(1), s);
        swr(self.status(2), s >> 4);
    }

    /// Zero: all digits, sign and scale.
    #[inline(always)]
    pub fn clear(self) {
        prim::clear(self.0)
    }

    /// `self = src` (digits, sign, scale).
    #[inline(always)]
    pub fn copy_from(self, src: Num) {
        prim::cp(self.0 + (src.0 >> 4))
    }

    #[inline(always)]
    pub fn is_zero(self) -> bool {
        self.digits() == 0
    }

    /// Number of significant digits (0 for zero).
    #[inline(always)]
    pub fn digits(self) -> u8 {
        prim::digits(self.0)
    }

    /// Digits printed: significant digits, but never fewer than `scale + 1`.
    #[inline(always)]
    pub fn width(self) -> u8 {
        prim::width(self.0)
    }

    /// Integer-part digit count (digits left of the point); 0 when `|self|<1`.
    #[inline(always)]
    pub fn int_digits(self) -> u8 {
        self.digits().saturating_sub(self.scale())
    }

    /// Multiply by 10^k. Returns `false` (value unchanged) if digits would be lost.
    #[inline(always)]
    pub fn shl10(self, k: u8) -> bool {
        prim::shl(self.0, k) != 0
    }

    /// Divide by 10^k, truncating. Returns the most significant dropped digit.
    #[inline(always)]
    pub fn shr10(self, k: u8) -> u8 {
        prim::shr(self.0, k)
    }

    /// Change the scale to `s`, appending zeros or truncating. Returns `false`
    /// (value unchanged) only when appending zeros would overflow.
    #[inline(always)]
    pub fn rescale(self, s: u8) -> bool {
        let cur = self.scale();
        if s >= cur {
            if !self.shl10(s - cur) {
                return false;
            }
        } else {
            self.shr10(cur - s);
        }
        self.set_scale(s);
        true
    }

    /// `dst = a ± b` (sign of `b` flipped when `negate`). Both operands are
    /// brought to the larger scale. Returns `false` -- `dst` unchanged -- if
    /// the result does not fit [`CAPACITY`] digits. `dst` may alias `a`/`b`.
    #[inline(always)]
    pub fn add_signed(dst: Num, a: Num, b: Num, negate: bool) -> bool {
        #[cfg(target_os = "none")]
        #[allow(unused_unsafe)]
        return unsafe { prim::entry::__n_add(dst.0 + negate as u8, a.0, b.0) } != 0;
        #[cfg(not(target_os = "none"))]
        Self::add_host(dst, a, b, negate)
    }

    /// The Rust form of [`Num::add_signed`] (host; the twin of `__n_add`).
    #[cfg(not(target_os = "none"))]
    #[inline(never)]
    pub fn add_host(dst: Num, a: Num, b: Num, negate: bool) -> bool {
        XA.copy_from(a);
        XB.copy_from(b);
        XB.set_neg(XB.is_neg() ^ negate);
        let (sa, sb) = (XA.scale(), XB.scale());
        let s = if sa > sb { sa } else { sb };
        if !XA.rescale(s) || !XB.rescale(s) {
            return false;
        }
        let mut r = XA;
        if XA.is_neg() == XB.is_neg() {
            if prim::add16(XA.0, XB.0) != 0 {
                return false;
            }
        } else if prim::cmp(XA.0, XB.0) != 0 {
            prim::sub16(XA.0, XB.0);
        } else {
            prim::sub16(XB.0, XA.0);
            r = XB;
        }
        if r.is_zero() {
            r.set_neg(false);
        }
        if r.width() > CAPACITY {
            return false;
        }
        dst.copy_from(r);
        true
    }

    /// `dst = a * b`, exact, natural scale `a.scale + b.scale`. `false` -- `dst`
    /// unchanged -- if the product needs more than 16 digits.
    #[inline(always)]
    pub fn mul(dst: Num, a: Num, b: Num) -> bool {
        #[cfg(target_os = "none")]
        #[allow(unused_unsafe)]
        return unsafe { prim::entry::__n_mul(dst.0, a.0, b.0) } != 0;
        #[cfg(not(target_os = "none"))]
        Self::mul_host(dst, a, b)
    }

    /// The Rust form of [`Num::mul`] (host; the twin of `__n_mul`).
    #[cfg(not(target_os = "none"))]
    #[inline(never)]
    pub fn mul_host(dst: Num, a: Num, b: Num) -> bool {
        prim::b_clear(MULBUF, 32u8.wrapping_neg());
        let mut i = 0;
        while i < DIGITS {
            let mut k = a.digit(i);
            while k > 0 {
                // The product of two 16-digit magnitudes is < 10^32, so the
                // carry is always absorbed inside the 32-digit buffer.
                prim::b_addsh(MULBUF + i, b.0);
                k -= 1;
            }
            i += 1;
        }
        if prim::digits(MULBUF + DIGITS) != 0 {
            return false;
        }
        let neg = a.is_neg() != b.is_neg();
        let s = a.scale() + b.scale();
        dst.set_from_buf(MULBUF, neg, s);
        true
    }

    /// Restoring long division `dst = a / b`, truncated: `frac` fractional
    /// digits, or [`FLOAT`] for the floating 13-significant-digit result (the
    /// integer part is always kept). `false` on divide-by-zero or when the
    /// quotient does not fit; `dst` is then unchanged.
    ///
    /// ASSUMPTION (spec M2-D1): FL division keeps 13 significant digits,
    /// truncated. Verified against the recovered ROM for examples 3-1, 3-4, 4-2.
    #[inline(always)]
    pub fn div(dst: Num, a: Num, b: Num, frac: u8) -> bool {
        // Target: the whole operation is `__n_div` in asm/num.s (flags in the
        // handles' character nibbles); this Rust body is its host twin.
        #[cfg(target_os = "none")]
        {
            let (d, a) = if frac == FLOAT {
                (dst.0, a.0 + 1)
            } else {
                (dst.0 + frac, a.0)
            };
            #[allow(unused_unsafe)]
            return unsafe { prim::entry::__n_div(d, a, b.0) } != 0;
        }
        #[cfg(not(target_os = "none"))]
        Self::div_host(dst, a, b, frac)
    }

    /// The Rust form of [`Num::div`] (host; the twin of `__n_div`).
    ///
    /// `R = floor(|a / b| * 10^fr)` with `fr = frac`, or in FL `fr = 13 -
    /// (integer digits of the quotient)` clamped to 0..=13; `false` when `b`
    /// is zero or `R` needs more than 16 digits. Long division of `A * 10^k`
    /// by `B` (the digit strings, `k = fr + sb - sa`): one quotient digit per
    /// step from the top, through a 17-digit remainder at [`DIVREM`], shifted
    /// into WIN. Quotient digit `j` weighs `10^(n - 1 - j + sb - sa)` in
    /// `a / b`, so in FL the first nonzero one fixes `fr` and the step count.
    ///
    /// ASSUMPTION (spec M2-D2): a quotient too small for FL's 13 places prints
    /// 0 (the M2 code reported overflow when its fixed buffer ran out).
    #[cfg(not(target_os = "none"))]
    #[inline(never)]
    pub fn div_host(dst: Num, a: Num, b: Num, frac: u8) -> bool {
        if b.is_zero() {
            return false;
        }
        let n = a.digits();
        let base = n as i8 + b.scale() as i8 - a.scale() as i8;
        let fl = frac == FLOAT;
        let mut fr = if fl { FLOAT_SIG } else { frac };
        let mut steps = base + fr as i8;
        prim::b_clear(DIVREM, 17u8.wrapping_neg());
        prim::b_clear(WIN.0, DIGITS.wrapping_neg());
        let mut lead = false;
        let mut j: i8 = 0;
        while j < steps {
            prim::b_up(DIVREM, 17u8.wrapping_neg());
            let d = if (j as u8) < n {
                a.digit(n - 1 - j as u8)
            } else {
                0
            };
            wr(DIVREM, d);
            let count = prim::b_divdig(b.0);
            if fl && !lead && count != 0 {
                lead = true;
                let int_digits = base - j; // digits left of the point
                fr = if int_digits < 0 {
                    FLOAT_SIG
                } else {
                    FLOAT_SIG.saturating_sub(int_digits as u8)
                };
                steps = base + fr as i8;
            }
            if WIN.digit(DIGITS - 1) != 0 {
                return false;
            }
            prim::b_up(WIN.0, DIGITS.wrapping_neg());
            WIN.set_digit(0, count);
            j += 1;
        }
        dst.set_from_buf(WIN.0, a.is_neg() != b.is_neg(), fr);
        true
    }

    /// `dst = a` to the DP selector's fractional digits, rounded half up when
    /// the rounding switch is 5/4, else truncated (both read from the latched
    /// selectors, `crate::SW_DP` / `SW_ROUND`). Returns whether it rounded up
    /// (drives the `^` mark).
    #[inline(always)]
    pub fn round_dp(dst: Num, a: Num) -> bool {
        #[cfg(target_os = "none")]
        #[allow(unused_unsafe)]
        return unsafe { prim::entry::__n_round(dst.0, a.0) } != 0;
        #[cfg(not(target_os = "none"))]
        Self::round_host(dst, a, srd(crate::SW_DP), srd(crate::SW_ROUND) == 1)
    }

    /// The Rust form of [`Num::round_dp`] with explicit `frac` and 5/4 switch
    /// (host; the twin of `__n_round`).
    /// `frac` < 16.
    #[cfg(not(target_os = "none"))]
    #[inline(never)]
    pub fn round_host(dst: Num, a: Num, frac: u8, round: bool) -> bool {
        if dst != a {
            dst.copy_from(a);
        }
        let s = dst.scale();
        if frac >= s {
            dst.rescale(frac);
            return false;
        }
        let first = dst.shr10(s - frac);
        dst.set_scale(frac);
        let mut rounded = false;
        if round && first >= 5 {
            rounded = prim::inc(dst.0) == 0;
        }
        if dst.is_zero() {
            dst.set_neg(false);
        }
        rounded
    }

    /// `dst = sqrt(|a|)` to `frac` fractional digits, truncated (digit-by-digit,
    /// firmware §4.5). Each root digit `x` is the number of successive odd
    /// terms `20r+1, 20r+3, ...` that can be subtracted from the remainder.
    #[inline(always)]
    pub fn sqrt_at(dst: Num, a: Num, frac: u8) {
        #[cfg(target_os = "none")]
        #[allow(unused_unsafe)]
        unsafe {
            prim::entry::__n_sqrt(dst.0, a.0, frac)
        }
        #[cfg(not(target_os = "none"))]
        Self::sqrt_host(dst, a, frac)
    }

    /// The Rust form of [`Num::sqrt_at`] (host; the twin of `__n_sqrt`).
    /// `frac` < 16.
    #[cfg(not(target_os = "none"))]
    #[inline(never)]
    pub fn sqrt_host(dst: Num, a: Num, frac: u8) {
        // Radicand digit k is a.digit(k - off), off = 2*frac - a.scale.
        let off = (2 * frac) as i8 - a.scale() as i8;
        let mut hi = 0u8;
        let mut i = 0;
        while i < DIGITS {
            let idx = i as i8 + off;
            if a.digit(i) != 0 && idx >= 0 && idx < WIDE as i8 && idx as u8 > hi {
                hi = idx as u8;
            }
            i += 1;
        }
        prim::b_clear(SQ_REM, (2 * SQ_LEN).wrapping_neg()); // remainder, odd term
        wr(SQ_S, 1);
        prim::b_clear(WIN.0, DIGITS.wrapping_neg()); // root
        let mut p = hi / 2 + 1;
        while p > 0 {
            p -= 1;
            // remainder = remainder * 100 + next digit pair
            prim::b_up(SQ_REM, SQ_LEN.wrapping_neg());
            prim::b_up(SQ_REM, SQ_LEN.wrapping_neg());
            wr(SQ_REM + 1, rad_digit(a, off, 2 * p + 1));
            wr(SQ_REM, rad_digit(a, off, 2 * p));
            let x = prim::b_sqdig();
            // root = root * 10 + x (the top digit drops, as the old 16-digit
            // root array did)
            prim::b_up(WIN.0, DIGITS.wrapping_neg());
            WIN.set_digit(0, x);
            // S = 20*(10r + x) + 1 = (S - 1) * 10 + 1  (S is odd)
            wr(SQ_S, rd(SQ_S) - 1);
            prim::b_up(SQ_S, SQ_LEN.wrapping_neg());
            wr(SQ_S, 1);
        }
        dst.set_from_buf(WIN.0, false, frac);
    }

    /// Set `self` from the 16 digits of a register-aligned buffer, with sign
    /// and scale; a zero magnitude gets sign + and scale 0.
    #[inline(always)]
    fn set_from_buf(self, buf: u8, neg: bool, scale: u8) {
        prim::cpyd(self.0, buf);
        if self.is_zero() {
            self.set_neg(false);
            self.set_scale(0);
        } else {
            self.set_neg(neg);
            self.set_scale(scale);
        }
    }
}

/// Radicand digit `k`: `a.digit(k - off)` when that index is a digit, else 0.
#[inline(always)]
fn rad_digit(a: Num, off: i8, k: u8) -> u8 {
    let i = k as i8 - off;
    if i >= 0 && i < DIGITS as i8 && (k as i8) < WIDE as i8 {
        a.digit(i as u8)
    } else {
        0
    }
}
