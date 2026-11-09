//! Number <-> text (size probe): FIN (parse a constant or VAL), FOUT (PRINT / STR$ form) and
//! PUSING (PRINT USING numeric fields).
use crate::mbf::{Dbl, Fault, Sng, R};

#[derive(Clone, Copy)]
pub enum Num {
    Int(i16),
    Sng(Sng),
    Dbl(Dbl),
}

/// Ten as a double, and powers of ten by repeated multiply.
fn ten_d() -> Dbl {
    Dbl::from_i16(10)
}

#[inline(never)]
fn pow10(mut e: i16) -> R<Dbl> {
    let mut r = Dbl::from_i16(1);
    let neg = e < 0;
    if neg {
        e = -e;
    }
    while e > 0 {
        r = r.mul(ten_d())?;
        e -= 1;
    }
    if neg {
        Dbl::from_i16(1).div(r)
    } else {
        Ok(r)
    }
}

/// FIN: parse at `s`, returns the number and the bytes used. Type from suffix / digits as
/// the original: more than 7 digits or D exponent or # makes a double, % an integer, ! single.
#[inline(never)]
pub fn fin(s: &[u8]) -> R<(Num, usize)> {
    let mut i = 0;
    while i < s.len() && s[i] == b' ' {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        neg = s[i] == b'-';
        i += 1;
    }
    if i + 1 < s.len() && s[i] == b'&' {
        let (radix, mut j) = match s[i + 1] {
            b'H' | b'h' => (16u32, i + 2),
            b'O' | b'o' => (8, i + 2),
            _ => (8, i + 1),
        };
        let mut v: u32 = 0;
        while j < s.len() {
            let d = match s[j] {
                c @ b'0'..=b'9' => (c - b'0') as u32,
                c @ b'A'..=b'F' if radix == 16 => (c - b'A' + 10) as u32,
                c @ b'a'..=b'f' if radix == 16 => (c - b'a' + 10) as u32,
                _ => break,
            };
            if d >= radix {
                break;
            }
            v = v * radix + d;
            if v > 0xFFFF {
                return Err(Fault::Overflow);
            }
            j += 1;
        }
        return Ok((Num::Int(v as u16 as i16), j));
    }
    let mut mant = Dbl(0);
    let mut digits = 0u8;
    let mut exp10: i16 = 0;
    let mut seen_point = false;
    let mut dbl = false;
    let mut force_sng = false;
    let mut int_ok = true;
    while i < s.len() {
        let c = s[i];
        if c.is_ascii_digit() {
            mant = mant.mul(ten_d())?.add(Dbl::from_i16((c - b'0') as i16))?;
            if digits > 0 || c != b'0' {
                digits += 1;
            }
            if seen_point {
                exp10 -= 1;
            }
        } else if c == b'.' && !seen_point {
            seen_point = true;
            int_ok = false;
        } else if c == b' ' {
        } else {
            break;
        }
        i += 1;
    }
    if i < s.len() && matches!(s[i], b'E' | b'e' | b'D' | b'd') {
        dbl = matches!(s[i], b'D' | b'd');
        int_ok = false;
        i += 1;
        let mut eneg = false;
        if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
            eneg = s[i] == b'-';
            i += 1;
        }
        let mut e: i16 = 0;
        while i < s.len() && s[i].is_ascii_digit() {
            e = e.saturating_mul(10).saturating_add((s[i] - b'0') as i16);
            i += 1;
        }
        exp10 += if eneg { -e } else { e };
    }
    if i < s.len() {
        match s[i] {
            b'#' => {
                dbl = true;
                i += 1
            }
            b'!' => {
                force_sng = true;
                i += 1
            }
            b'%' => {
                i += 1;
                let v = mant.to_sng()?.to_i16()?;
                return Ok((Num::Int(if neg { -v } else { v }), i));
            }
            _ => {}
        }
    }
    if digits > 7 && !force_sng {
        dbl = true;
    }
    let mut v = if exp10 != 0 {
        mant.mul(pow10(exp10)?)?
    } else {
        mant
    };
    if neg {
        v = v.negate();
    }
    if dbl {
        return Ok((Num::Dbl(v), i));
    }
    let sv = v.to_sng()?;
    if int_ok && !force_sng {
        if let Ok(n) = sv.to_i16() {
            if sv.cmp(Sng::from_i16(n)) == 0 && !(n == -32768 && !neg) {
                return Ok((Num::Int(n), i));
            }
        }
    }
    Ok((Num::Sng(sv), i))
}

/// Decimal digits of a positive value: `n` significant digits, returns exponent of the first.
#[inline(never)]
fn digits_of(v: Dbl, n: u8, out: &mut [u8; 17]) -> i16 {
    // scale into [10^(n-1), 10^n)
    let mut e: i16 = 0;
    let mut x = v;
    let lo = pow10((n - 1) as i16).unwrap_or(Dbl(0));
    let hi = pow10(n as i16).unwrap_or(Dbl(0));
    let ge = |a: Dbl, b: Dbl| a.exp() > b.exp() || (a.exp() == b.exp() && a.man() >= b.man());
    while !ge(x, lo) {
        x = x.mul(ten_d()).unwrap_or(x);
        e -= 1;
    }
    while ge(x, hi) {
        x = x.div(ten_d()).unwrap_or(x);
        e += 1;
    }
    // round to integer
    let half = Dbl::from_i16(1).div(Dbl::from_i16(2)).unwrap_or(Dbl(0));
    x = x.add(half).unwrap_or(x);
    let sh = 56 - (x.exp() - 128);
    let mut int = if sh >= 64 { 0 } else { x.man() >> sh };
    let mut k = n as usize;
    if int >= 10u64.pow(n as u32) {
        int /= 10;
        e += 1;
    }
    while k > 0 {
        k -= 1;
        out[k] = b'0' + (int % 10) as u8;
        int /= 10;
    }
    e + n as i16 - 1
}

/// FOUT: the text PRINT shows for a number (leading space or minus, trailing space is the
/// caller's).
#[inline(never)]
pub fn fout(v: Num, buf: &mut [u8; 32]) -> usize {
    let mut p = 0;
    let (neg, d, n, ch) = match v {
        Num::Int(i) => {
            buf[0] = if i < 0 { b'-' } else { b' ' };
            let mut u = (i as i32).unsigned_abs();
            let mut t = [0u8; 6];
            let mut k = 0;
            loop {
                t[k] = b'0' + (u % 10) as u8;
                k += 1;
                u /= 10;
                if u == 0 {
                    break;
                }
            }
            p = 1;
            while k > 0 {
                k -= 1;
                buf[p] = t[k];
                p += 1;
            }
            return p;
        }
        Num::Sng(s) => (s.neg(), s.to_dbl(), 7u8, b'E'),
        Num::Dbl(d) => (d.neg(), d, 16u8, b'D'),
    };
    buf[p] = if neg { b'-' } else { b' ' };
    p += 1;
    if d.is_zero() {
        buf[p] = b'0';
        return p + 1;
    }
    let mut dg = [0u8; 17];
    let e = digits_of(if neg { d.negate() } else { d }, n, &mut dg);
    let mut last = n as usize;
    while last > 1 && dg[last - 1] == b'0' {
        last -= 1;
    }
    if e >= n as i16 || e < -(n as i16) - 1 + (last as i16) - 1 {
        // scientific
        buf[p] = dg[0];
        p += 1;
        if last > 1 {
            buf[p] = b'.';
            p += 1;
            for &c in &dg[1..last] {
                buf[p] = c;
                p += 1;
            }
        }
        buf[p] = ch;
        buf[p + 1] = if e < 0 { b'-' } else { b'+' };
        let ae = e.unsigned_abs();
        buf[p + 2] = b'0' + (ae / 10) as u8;
        buf[p + 3] = b'0' + (ae % 10) as u8;
        return p + 4;
    }
    if e < 0 {
        buf[p] = b'.';
        p += 1;
        for _ in 0..(-e - 1) {
            buf[p] = b'0';
            p += 1;
        }
        for &c in &dg[..last] {
            buf[p] = c;
            p += 1;
        }
    } else {
        for (k, &c) in dg[..last.max(e as usize + 1)].iter().enumerate() {
            if k == e as usize + 1 {
                buf[p] = b'.';
                p += 1;
            }
            buf[p] = c;
            p += 1;
        }
    }
    p
}

/// A PRINT USING numeric field.
pub struct Field {
    pub before: u8,
    pub after: u8,
    pub comma: bool,
    pub dollar: bool,
    pub star: bool,
    pub plus: bool,
    pub minus_after: bool,
    pub exp: bool,
}

#[inline(never)]
pub fn pusing(f: &Field, v: Num, out: &mut [u8; 40]) -> usize {
    let d = match v {
        Num::Int(i) => Dbl::from_i16(i),
        Num::Sng(s) => s.to_dbl(),
        Num::Dbl(d) => d,
    };
    let neg = d.neg();
    let a = if neg { d.negate() } else { d };
    let mut dg = [b'0'; 17];
    let tot = (f.before + f.after).clamp(1, 16);
    let mut p = 0;
    if a.is_zero() {
        for _ in 0..f.before {
            out[p] = b' ';
            p += 1;
        }
        return p;
    }
    let e = digits_of(a, tot, &mut dg);
    if f.exp {
        if f.plus {
            out[p] = if neg { b'-' } else { b'+' };
            p += 1;
        } else if neg {
            out[p] = b'-';
            p += 1;
        }
        for k in 0..f.before as usize {
            out[p] = dg[k];
            p += 1;
        }
        out[p] = b'.';
        p += 1;
        for k in 0..f.after as usize {
            out[p] = dg[f.before as usize + k];
            p += 1;
        }
        let x = e - f.before as i16 + 1;
        out[p] = b'E';
        out[p + 1] = if x < 0 { b'-' } else { b'+' };
        out[p + 2] = b'0' + (x.unsigned_abs() / 10) as u8;
        out[p + 3] = b'0' + (x.unsigned_abs() % 10) as u8;
        return p + 4;
    }
    let int_digits = (e + 1).max(1) as usize;
    let width = f.before as usize
        + if f.comma {
            (f.before as usize - 1) / 3
        } else {
            0
        };
    let mut text = [0u8; 40];
    let mut t = 0;
    if neg && !f.minus_after && !f.plus {
        text[t] = b'-';
        t += 1;
    }
    if f.plus {
        text[t] = if neg { b'-' } else { b'+' };
        t += 1;
    }
    if f.dollar {
        text[t] = b'$';
        t += 1;
    }
    for k in 0..int_digits {
        if f.comma && k > 0 && (int_digits - k) % 3 == 0 {
            text[t] = b',';
            t += 1;
        }
        text[t] = if e < 0 { b'0' } else { dg[k] };
        t += 1;
    }
    if f.after > 0 {
        text[t] = b'.';
        t += 1;
        for k in 0..f.after as usize {
            let idx = int_digits + k;
            text[t] = if e < 0 { b'0' } else { dg[idx.min(16)] };
            t += 1;
        }
    }
    if t > width + f.after as usize + 1 {
        out[p] = b'%';
        p += 1;
    } else {
        let fill = if f.star { b'*' } else { b' ' };
        for _ in t..width + if f.after > 0 { f.after as usize + 1 } else { 0 } {
            out[p] = fill;
            p += 1;
        }
    }
    for &c in &text[..t] {
        out[p] = c;
        p += 1;
    }
    if f.minus_after {
        out[p] = if neg { b'-' } else { b' ' };
        p += 1;
    }
    p
}
