//! Microsoft Binary Format arithmetic (size probe). Single: 4 bytes, exponent byte (bias 128,
//! 0 = zero), sign in bit 7 of the mantissa's top byte, hidden leading 1. Double: 8 bytes, the
//! same with 56 mantissa bits. Arithmetic keeps a guard byte and rounds half up like the
//! original's FADD/FMULT (simplified).

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Sng(pub u32); // exp << 24 | sign << 23 | mantissa (23 bits)

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Dbl(pub u64);

pub enum Fault {
    Overflow,
    DivZero,
}

pub type R<T> = Result<T, Fault>;

#[inline(never)]
fn pack_s(neg: bool, mut exp: i16, mut m: u32) -> R<Sng> {
    // m: 32-bit with leading one at bit 31 (or zero)
    if m == 0 {
        return Ok(Sng(0));
    }
    while m & 0x8000_0000 == 0 {
        m <<= 1;
        exp -= 1;
    }
    // round at bit 7 (24 significant bits)
    let r = m & 0xFF;
    m >>= 8;
    if r >= 0x80 {
        m += 1;
        if m & 0x0100_0000 != 0 {
            m >>= 1;
            exp += 1;
        }
    }
    if exp <= 0 {
        return Ok(Sng(0));
    }
    if exp > 255 {
        return Err(Fault::Overflow);
    }
    Ok(Sng(((exp as u32) << 24)
        | ((neg as u32) << 23)
        | (m & 0x7F_FFFF)))
}

impl Sng {
    pub fn exp(self) -> i16 {
        (self.0 >> 24) as i16
    }
    pub fn neg(self) -> bool {
        self.0 & 0x80_0000 != 0
    }
    pub fn man(self) -> u32 {
        (self.0 & 0x7F_FFFF) | 0x80_0000
    }
    pub fn is_zero(self) -> bool {
        self.exp() == 0
    }
    #[inline(never)]
    pub fn add(self, o: Sng) -> R<Sng> {
        if self.is_zero() {
            return Ok(o);
        }
        if o.is_zero() {
            return Ok(self);
        }
        let (a, b) = if self.exp() >= o.exp() {
            (self, o)
        } else {
            (o, self)
        };
        let d = a.exp() - b.exp();
        if d > 24 {
            return Ok(a);
        }
        let ma = a.man() << 7;
        let mb = (b.man() << 7) >> d;
        if a.neg() == b.neg() {
            pack_s(a.neg(), a.exp() + 1, ma + mb)
        } else if ma >= mb {
            pack_s(a.neg(), a.exp() + 1, ma - mb)
        } else {
            pack_s(b.neg(), a.exp() + 1, mb - ma)
        }
    }
    pub fn negate(self) -> Sng {
        if self.is_zero() {
            self
        } else {
            Sng(self.0 ^ 0x80_0000)
        }
    }
    pub fn sub(self, o: Sng) -> R<Sng> {
        self.add(o.negate())
    }
    #[inline(never)]
    pub fn mul(self, o: Sng) -> R<Sng> {
        if self.is_zero() || o.is_zero() {
            return Ok(Sng(0));
        }
        let p = (self.man() as u64) * (o.man() as u64); // 48 bits
        let m = (p >> 16) as u32;
        pack_s(self.neg() != o.neg(), self.exp() + o.exp() - 128, m)
    }
    #[inline(never)]
    pub fn div(self, o: Sng) -> R<Sng> {
        if o.is_zero() {
            return Err(Fault::DivZero);
        }
        if self.is_zero() {
            return Ok(Sng(0));
        }
        let n = (self.man() as u64) << 32;
        let q = n / (o.man() as u64);
        let mut e = self.exp() - o.exp() + 128 + 8;
        let mut q = q as u64;
        while q >= 1u64 << 32 {
            q >>= 1;
            e += 1;
        }
        pack_s(self.neg() != o.neg(), e, q as u32)
    }
    #[inline(never)]
    pub fn cmp(self, o: Sng) -> i8 {
        let k = |s: Sng| -> i64 {
            if s.is_zero() {
                0
            } else {
                let v = ((s.exp() as i64) << 24) | (s.man() as i64);
                if s.neg() {
                    -v
                } else {
                    v
                }
            }
        };
        let (a, b) = (k(self), k(o));
        (a > b) as i8 - (a < b) as i8
    }
    #[inline(never)]
    pub fn from_i16(v: i16) -> Sng {
        let neg = v < 0;
        let m = (v as i32).unsigned_abs() << 16;
        pack_s(neg, 128 + 16, m).unwrap_or(Sng(0))
    }
    /// CINT: round half away from zero (the original rounds .5 up in magnitude)
    #[inline(never)]
    pub fn to_i16(self) -> R<i16> {
        if self.is_zero() {
            return Ok(0);
        }
        let e = self.exp() - 128;
        if e > 16 {
            return Err(Fault::Overflow);
        }
        if e < 0 {
            return Ok(0);
        }
        let m = (self.man() as u64) << 8; // 32 bits, point after bit 31-e...
        let sh = 32 - e;
        let mut v = (m >> sh) as i32;
        if sh > 0 && (m >> (sh - 1)) & 1 != 0 {
            v += 1;
        }
        let v = if self.neg() { -v } else { v };
        if !(-32768..=32767).contains(&v) {
            return Err(Fault::Overflow);
        }
        Ok(v as i16)
    }
    /// INT: floor
    #[inline(never)]
    pub fn floor(self) -> Sng {
        let e = self.exp() - 128;
        if self.is_zero() || e >= 24 {
            return self;
        }
        if e <= 0 {
            return if self.neg() {
                Sng::from_i16(-1)
            } else {
                Sng(0)
            };
        }
        let mask = 0xFF_FFFFu32 >> e;
        let frac = self.man() & mask;
        let t = Sng((self.0 & !mask) | 0x0);
        let t = Sng(t.0 & !(mask & 0x7F_FFFF));
        if frac != 0 && self.neg() {
            t.sub(Sng::from_i16(1)).unwrap_or(t)
        } else {
            t
        }
    }
    pub fn to_dbl(self) -> Dbl {
        Dbl(((self.0 as u64) << 32))
    }
}

#[inline(never)]
fn pack_d(neg: bool, mut exp: i16, mut hi: u64, lo: u8) -> R<Dbl> {
    if hi == 0 {
        return Ok(Dbl(0));
    }
    while hi & (1 << 63) == 0 {
        hi = (hi << 1) | ((lo >> 7) as u64);
        exp -= 1;
    }
    let r = hi & 0xFF;
    hi >>= 8;
    if r >= 0x80 {
        hi += 1;
        if hi & (1 << 56) != 0 {
            hi >>= 1;
            exp += 1;
        }
    }
    if exp <= 0 {
        return Ok(Dbl(0));
    }
    if exp > 255 {
        return Err(Fault::Overflow);
    }
    Ok(Dbl(((exp as u64) << 56)
        | ((neg as u64) << 55)
        | (hi & 0x7F_FFFF_FFFF_FFFF)))
}

impl Dbl {
    pub fn exp(self) -> i16 {
        (self.0 >> 56) as i16
    }
    pub fn neg(self) -> bool {
        self.0 & (1 << 55) != 0
    }
    pub fn man(self) -> u64 {
        (self.0 & 0x7F_FFFF_FFFF_FFFF) | (1 << 55)
    }
    pub fn is_zero(self) -> bool {
        self.exp() == 0
    }
    pub fn negate(self) -> Dbl {
        if self.is_zero() {
            self
        } else {
            Dbl(self.0 ^ (1 << 55))
        }
    }
    #[inline(never)]
    pub fn add(self, o: Dbl) -> R<Dbl> {
        if self.is_zero() {
            return Ok(o);
        }
        if o.is_zero() {
            return Ok(self);
        }
        let (a, b) = if self.exp() >= o.exp() {
            (self, o)
        } else {
            (o, self)
        };
        let d = a.exp() - b.exp();
        if d > 56 {
            return Ok(a);
        }
        let ma = a.man() << 7;
        let mb = (b.man() << 7) >> d;
        if a.neg() == b.neg() {
            pack_d(a.neg(), a.exp() + 1, ma + mb, 0)
        } else if ma >= mb {
            pack_d(a.neg(), a.exp() + 1, ma - mb, 0)
        } else {
            pack_d(b.neg(), a.exp() + 1, mb - ma, 0)
        }
    }
    pub fn sub(self, o: Dbl) -> R<Dbl> {
        self.add(o.negate())
    }
    #[inline(never)]
    pub fn mul(self, o: Dbl) -> R<Dbl> {
        if self.is_zero() || o.is_zero() {
            return Ok(Dbl(0));
        }
        // 56x56 by 28-bit halves, keep the top 64 bits
        let (a, b) = (self.man(), o.man());
        let (ah, al) = (a >> 28, a & 0xFFF_FFFF);
        let (bh, bl) = (b >> 28, b & 0xFFF_FFFF);
        let hh = ah * bh;
        let mid = ah * bl + al * bh + ((al * bl) >> 28);
        let p = hh + (mid >> 28); // product >> 56, 56 bits
        pack_d(
            self.neg() != o.neg(),
            self.exp() + o.exp() - 128,
            p << 8,
            (mid >> 20) as u8,
        )
    }
    #[inline(never)]
    pub fn div(self, o: Dbl) -> R<Dbl> {
        if o.is_zero() {
            return Err(Fault::DivZero);
        }
        if self.is_zero() {
            return Ok(Dbl(0));
        }
        // restoring division, 64 quotient bits
        let (mut n, d) = (self.man(), o.man());
        let mut q: u64 = 0;
        for _ in 0..64 {
            q <<= 1;
            if n >= d {
                n -= d;
                q |= 1;
            }
            n <<= 1;
        }
        pack_d(self.neg() != o.neg(), self.exp() - o.exp() + 129, q, 0)
    }
    pub fn to_sng(self) -> R<Sng> {
        let m = (self.man() >> 24) as u32;
        pack_s(self.neg(), self.exp(), m << 0)
    }
    pub fn from_i16(v: i16) -> Dbl {
        Sng::from_i16(v).to_dbl()
    }
}
