//! SCELBAL's floating point package (listing pages 20-25 and 06-07).
//!
//! A number is four bytes: a 24-bit two's complement mantissa (LSW, NSW,
//! MSW; normalized so bit 6 of the MSW is the leading one) and a two's
//! complement exponent. The value is mantissa / 2^23 * 2^exponent, so +1.0
//! is 000 000 100 001. The registers live on page 01 at the listing's
//! offsets; every routine here works on them the way the original does,
//! byte by byte, so results (rounding, truncation, the decimal conversions)
//! are the original's to the bit.
//!
//! Page 01 offsets used here (octal):
//!   100 sign  101 M/D signs  102 bit counter  103-106 DINPUT flags
//!   107 output digit counter  110 fixed/float output flag
//!   123-127 FPACC (extension, LSW, NSW, MSW, exponent)
//!   130-137 FPOP (extension 130-133, LSW..exponent 134-137)
//!   140-147 partial product / quotient, 150-157 DINPUT, 160-167 FPOUT
//!   210 +10.0  214 +0.1  220 GETINP counter

use crate::mem::{at, g1, moveit, rd, s1, wr, PG1};
use crate::{echo, fail, neg, Hal};

// The multi-byte carry primitives. The 8008 keeps the carry in a flag that
// compiled Rust cannot reach, so the target uses asm/fp.s (spec "Target"); the
// host uses these Rust versions, which define the contracts.
#[cfg(not(target_os = "none"))]
use host::{adder, complm, rotl, rotr, subber};
#[cfg(target_os = "none")]
use target::{adder, complm, rotl, rotr, subber};

#[cfg(not(target_os = "none"))]
mod host {
    use crate::mem::{g1, s1};
    use crate::neg;

    /// ROTATL / ROTL: rotate `n` bytes of page 01 left through the carry,
    /// starting at the least significant byte `o`. Returns the carry out.
    #[inline(never)]
    pub fn rotl(mut o: u8, mut n: u8, mut cy: bool) -> bool {
        loop {
            let v = g1(o);
            s1(o, v << 1 | cy as u8);
            cy = neg(v);
            n = n.wrapping_sub(1);
            if n == 0 {
                return cy;
            }
            o = o.wrapping_add(1);
        }
    }

    /// ROTATR / ROTR: rotate right through the carry from the most significant
    /// byte `o` downwards. Returns the carry out.
    #[inline(never)]
    pub fn rotr(mut o: u8, mut n: u8, mut cy: bool) -> bool {
        loop {
            let v = g1(o);
            s1(o, v >> 1 | (cy as u8) << 7);
            cy = v & 1 != 0;
            n = n.wrapping_sub(1);
            if n == 0 {
                return cy;
            }
            o = o.wrapping_sub(1);
        }
    }

    /// COMPLM: two's complement of `n` bytes from the least significant `o`.
    #[inline(never)]
    pub fn complm(mut o: u8, mut n: u8) {
        let mut cy = true;
        loop {
            let (v, c) = (!g1(o)).overflowing_add(cy as u8);
            s1(o, v);
            cy = c;
            n = n.wrapping_sub(1);
            if n == 0 {
                return;
            }
            o = o.wrapping_add(1);
        }
    }

    /// ADDER: bytes at `b` += bytes at `a` (`n` bytes, carry chained).
    #[inline(never)]
    pub fn adder(mut a: u8, mut b: u8, mut n: u8) {
        let mut cy = false;
        loop {
            let (v, c1) = g1(a).overflowing_add(g1(b));
            let (v, c2) = v.overflowing_add(cy as u8);
            s1(b, v);
            cy = c1 | c2;
            n = n.wrapping_sub(1);
            if n == 0 {
                return;
            }
            a = a.wrapping_add(1);
            b = b.wrapping_add(1);
        }
    }

    /// SUBBER: bytes at `b` = bytes at `a` - bytes at `b`.
    #[inline(never)]
    pub fn subber(mut a: u8, mut b: u8, mut n: u8) {
        let mut cy = false;
        loop {
            let (v, c1) = g1(a).overflowing_sub(g1(b));
            let (v, c2) = v.overflowing_sub(cy as u8);
            s1(b, v);
            cy = c1 | c2;
            n = n.wrapping_sub(1);
            if n == 0 {
                return;
            }
            a = a.wrapping_add(1);
            b = b.wrapping_add(1);
        }
    }
}

#[cfg(target_os = "none")]
mod target {
    extern "C" {
        fn basic_rotl(o: u8, n: u8, cy: bool) -> bool;
        fn basic_rotr(o: u8, n: u8, cy: bool) -> bool;
        fn basic_complm(o: u8, n: u8);
        fn basic_adder(a: u8, b: u8, n: u8);
        fn basic_subber(a: u8, b: u8, n: u8);
    }
    #[inline(always)]
    pub fn rotl(o: u8, n: u8, cy: bool) -> bool {
        unsafe { basic_rotl(o, n, cy) }
    }
    #[inline(always)]
    pub fn rotr(o: u8, n: u8, cy: bool) -> bool {
        unsafe { basic_rotr(o, n, cy) }
    }
    #[inline(always)]
    pub fn complm(o: u8, n: u8) {
        unsafe { basic_complm(o, n) }
    }
    #[inline(always)]
    pub fn adder(a: u8, b: u8, n: u8) {
        unsafe { basic_adder(a, b, n) }
    }
    #[inline(always)]
    pub fn subber(a: u8, b: u8, n: u8) {
        unsafe { basic_subber(a, b, n) }
    }
}

/// MOVEIT within page 01.
#[inline(always)]
fn move1(src: u8, dst: u8, n: u8) {
    moveit(at(PG1, src), at(PG1, dst), n)
}

#[inline(always)]
fn dec1(o: u8) -> u8 {
    let v = g1(o).wrapping_sub(1);
    s1(o, v);
    v
}

#[inline(always)]
fn inc1(o: u8) -> u8 {
    let v = g1(o).wrapping_add(1);
    s1(o, v);
    v
}

/// FLOAD: four bytes at `src` into the FPACC.
#[inline(never)]
pub fn fload(src: u16) {
    moveit(src, at(PG1, 0o124), 4)
}

/// FSTORE: the FPACC into four bytes at `dst`.
#[inline(never)]
pub fn fstore(dst: u16) {
    moveit(at(PG1, 0o124), dst, 4)
}

/// OPLOAD: four bytes at `src` into the FPOP.
#[inline(never)]
pub fn opload(src: u16) {
    moveit(src, at(PG1, 0o134), 4)
}

/// FACXOP: FPOP = FPACC, then FPACC = four bytes at `src`.
#[inline(never)]
pub fn facxop(src: u16) {
    moveit(at(PG1, 0o124), at(PG1, 0o134), 4);
    moveit(src, at(PG1, 0o124), 4)
}

/// FPONE / CTRUE: FPACC = +1.0.
#[inline(never)]
pub fn fpone() {
    fload(at(PG1, 0o004))
}

/// FPZERO: clear the FPACC mantissa and extension (not the exponent).
#[inline(always)]
fn fpzero() {
    s1(0o126, 0);
    s1(0o125, 0);
    s1(0o124, 0);
    s1(0o123, 0);
}

/// CFALSE: FPACC = 0.
#[inline(never)]
pub fn cfalse() {
    s1(0o127, 0);
    fpzero()
}

/// FPCOMP: negate the FPACC mantissa.
#[inline(never)]
pub fn fpcomp() {
    complm(0o124, 3)
}

/// RESIGN: negate the FPACC again if the saved sign was negative.
#[inline(always)]
fn resign() {
    if neg(g1(0o100)) {
        complm(0o124, 3)
    }
}

/// FPFIX: FPACC to a 24-bit integer in LSW..MSW (truncated toward zero).
pub fn fpfix() {
    let a = g1(0o126);
    s1(0o100, a);
    if neg(a) {
        fpcomp();
    }
    let e = g1(0o127);
    if neg(e) {
        fpzero();
        return;
    }
    let mut c = 23u8.wrapping_sub(e);
    if neg(c) {
        fail(b'F', b'X');
    }
    loop {
        rotr(0o126, 3, false);
        c = c.wrapping_sub(1);
        if c == 0 {
            break;
        }
    }
    resign();
}

/// FPNORM: normalize the FPACC (extension included). `b` != 0 first sets
/// the exponent to `b` (FPFLT passes 23 to float an integer).
pub fn fpnorm(b: u8) {
    if b != 0 {
        s1(0o127, b);
    }
    let a = g1(0o126);
    s1(0o100, a);
    if neg(a) {
        complm(0o123, 4);
    }
    if g1(0o126) | g1(0o125) | g1(0o124) | g1(0o123) == 0 {
        s1(0o127, 0);
        return;
    }
    loop {
        rotl(0o123, 4, false);
        if neg(g1(0o126)) {
            break;
        }
        dec1(0o127);
    }
    rotr(0o126, 3, false);
    resign();
}

/// FPFLT: the 24-bit integer in the FPACC to floating point.
#[inline(always)]
pub fn fpflt() {
    fpnorm(23)
}

/// SHLOOP: shift a register's mantissa (with extension) right one place,
/// keeping the sign, and count it in its exponent at `exp`.
fn shloop(exp: u8) {
    inc1(exp);
    let m = exp - 1;
    rotr(m, 4, neg(g1(m)));
}

/// MOVOP: FPACC = FPOP.
#[inline(always)]
fn movop() {
    move1(0o134, 0o124, 4)
}

/// FPADD: FPACC = FPOP + FPACC.
pub fn fpadd() {
    if g1(0o126) == 0 {
        return movop();
    }
    if g1(0o136) == 0 {
        return;
    }
    let ea = g1(0o127);
    let eo = g1(0o137);
    if ea != eo {
        // `CPM` leaves the borrow of ea - eo in the carry, and `SBB` takes
        // it: the range test sees eo - ea - 1 when ea < eo (unsigned).
        let mut d = eo.wrapping_sub(ea).wrapping_sub((ea < eo) as u8);
        if neg(d) {
            d = 0u8.wrapping_sub(d);
        }
        let diff = eo.wrapping_sub(ea);
        if !neg(d.wrapping_sub(0o30)) {
            // Too far apart: the larger number is the sum.
            if !neg(diff) {
                movop();
            }
            return;
        }
        let mut c = diff;
        if neg(diff) {
            loop {
                shloop(0o137);
                c = c.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
        } else {
            loop {
                shloop(0o127);
                c = c.wrapping_sub(1);
                if c == 0 {
                    break;
                }
            }
        }
    }
    // PATCH 1: clear both extensions, then one more shift each for overflow.
    s1(0o123, 0);
    s1(0o133, 0);
    shloop(0o127);
    shloop(0o137);
    adder(0o133, 0o123, 4);
    fpnorm(0);
}

/// FPSUB: FPACC = FPOP - FPACC.
#[inline(always)]
pub fn fpsub() {
    fpcomp();
    fpadd()
}

/// CKSIGN: clear the work areas, make both operands positive and count
/// the negative ones in the M/D signs indicator (1 = result positive).
fn cksign() {
    let mut o = 0o140;
    while o < 0o150 {
        s1(o, 0);
        o += 1;
    }
    let mut o = 0o130;
    while o < 0o134 {
        s1(o, 0);
        o += 1;
    }
    s1(0o101, 1);
    if neg(g1(0o126)) {
        dec1(0o101);
        complm(0o124, 3);
    }
    if neg(g1(0o136)) {
        dec1(0o101);
        complm(0o134, 3);
    }
}

/// EXMLDV: the result (four bytes from `src`) into the FPACC, normalized,
/// with the sign from CKSIGN.
#[inline(always)]
fn exmldv(src: u8) {
    move1(src, 0o123, 4);
    fpnorm(0);
    if g1(0o101) == 0 {
        fpcomp()
    }
}

/// FPMULT: FPACC = FPOP * FPACC, shift-and-add over 23 bits, rounded on
/// the 24th.
pub fn fpmult() {
    cksign();
    s1(0o127, g1(0o137).wrapping_add(g1(0o127)).wrapping_add(1));
    s1(0o102, 23);
    loop {
        if rotr(0o126, 3, false) {
            adder(0o131, 0o141, 6);
        }
        rotr(0o146, 6, false);
        if dec1(0o102) == 0 {
            break;
        }
    }
    rotr(0o146, 6, false);
    if g1(0o143) & 0o100 != 0 {
        // MROUND
        let (v, mut cy) = g1(0o143).overflowing_add(0o100);
        s1(0o143, v);
        let mut o = 0o144;
        while o < 0o147 {
            let (v, c) = g1(o).overflowing_add(cy as u8);
            s1(o, v);
            cy = c;
            o += 1;
        }
    }
    exmldv(0o143)
}

/// SETSUB: working area = dividend - divisor; true if negative.
#[inline(always)]
fn setsub() -> bool {
    move1(0o124, 0o131, 3);
    subber(0o134, 0o131, 3);
    neg(g1(0o133))
}

/// FPDIV: FPACC = FPOP / FPACC, restoring division over 23 bits, rounded.
pub fn fpdiv() {
    cksign();
    if g1(0o126) == 0 {
        fail(b'D', b'Z');
    }
    s1(0o127, g1(0o137).wrapping_sub(g1(0o127)).wrapping_add(1));
    s1(0o102, 23);
    loop {
        let bit = !setsub();
        if bit {
            move1(0o131, 0o134, 3);
        }
        rotl(0o144, 3, bit);
        rotl(0o134, 3, false);
        if dec1(0o102) == 0 {
            break;
        }
    }
    if !setsub() {
        let (v, cy) = g1(0o144).overflowing_add(1);
        s1(0o144, v);
        let (v, cy) = g1(0o145).overflowing_add(cy as u8);
        s1(0o145, v);
        let v = g1(0o146).wrapping_add(cy as u8);
        s1(0o146, v);
        if neg(v) {
            rotr(0o146, 3, false);
            inc1(0o127);
        }
    }
    exmldv(0o143)
}

/// GETINP: the next character of the counted string at `buf`, 0 at its
/// end. The first call writes a zero byte after the string.
fn getinp(buf: u16) -> u8 {
    if g1(0o220) == 0 {
        let n = rd(buf).wrapping_add(1);
        wr(buf.wrapping_add(n as u16), 0);
    }
    let c = inc1(0o220);
    let a = rd(buf.wrapping_add(c as u16));
    if a == 0 {
        s1(0o220, 0);
    }
    a
}

/// DECBIN: conversion register (154..156) = 10 * itself + digit `c`.
fn decbin(c: u8) {
    s1(0o153, c & 0o17);
    move1(0o154, 0o150, 3);
    rotl(0o154, 3, false);
    rotl(0o154, 3, false);
    adder(0o150, 0o154, 3);
    rotl(0o154, 3, false);
    s1(0o152, 0);
    s1(0o151, 0);
    s1(0o150, g1(0o153));
    adder(0o150, 0o154, 3);
}

fn is_digit(c: u8) -> bool {
    !neg(c.wrapping_sub(0o260)) && neg(c.wrapping_sub(0o272))
}

/// NUMERR: the IN error.
#[inline(always)]
fn numerr() -> ! {
    s1(0o220, 0);
    fail(b'I', b'N')
}

/// DINPUT: the decimal string at `buf` (count first) into the FPACC.
pub fn dinput(buf: u16) {
    let mut o = 0o150;
    while o < 0o160 {
        s1(o, 0);
        o += 1;
    }
    let mut o = 0o103;
    while o < 0o107 {
        s1(o, 0);
        o += 1;
    }
    let mut c = getinp(buf);
    if c == 0o253 {
        c = getinp(buf);
    } else if c == 0o255 {
        s1(0o103, c);
        c = getinp(buf);
    }
    // Mantissa digits, an optional point, spaces ignored.
    let mut exp = false;
    loop {
        if c == 0o256 {
            if g1(0o106) != 0 {
                numerr();
            }
            s1(0o105, 0);
            s1(0o106, c);
        } else if c == 0o305 {
            exp = true;
            break;
        } else if c == 0 {
            break;
        } else if c != 0o240 {
            if !is_digit(c) {
                numerr();
            }
            // No room for another digit: the listing ignores it, uncounted.
            if g1(0o156) & 0o370 == 0 {
                inc1(0o105);
                decbin(c);
            }
        }
        c = getinp(buf);
    }
    if exp {
        c = getinp(buf);
        if c == 0o253 {
            c = getinp(buf);
        } else if c == 0o255 {
            s1(0o104, c);
            c = getinp(buf);
        }
        while c != 0 {
            if !is_digit(c) {
                numerr();
            }
            let m = g1(0o157);
            if neg(3u8.wrapping_sub(m)) {
                numerr();
            }
            s1(0o157, m.wrapping_mul(10).wrapping_add(c & 0o17));
            c = getinp(buf);
        }
    }
    // ENDINP
    if g1(0o103) != 0 {
        complm(0o154, 3);
    }
    s1(0o153, 0);
    move1(0o153, 0o123, 4);
    fpflt();
    if g1(0o104) != 0 {
        s1(0o157, 0u8.wrapping_sub(g1(0o157)));
    }
    let point = if g1(0o106) != 0 {
        0u8.wrapping_sub(g1(0o105))
    } else {
        0
    };
    let e = g1(0o157).wrapping_add(point);
    s1(0o157, e);
    if neg(e) {
        // FPD10
        loop {
            facxop(at(PG1, 0o214));
            fpmult();
            if inc1(0o157) == 0 {
                break;
            }
        }
    } else if e != 0 {
        // FPX10
        loop {
            facxop(at(PG1, 0o210));
            fpmult();
            if dec1(0o157) == 0 {
                break;
            }
        }
    }
}

/// OUTX10: output register (164..167) *= 10, pushing the top digit into 167.
fn outx10() {
    s1(0o167, 0);
    move1(0o164, 0o160, 4);
    rotl(0o164, 4, false);
    rotl(0o164, 4, false);
    adder(0o160, 0o164, 4);
    rotl(0o164, 4, false);
}

/// EXPOUT: `E+nn` / `E-nn` for a nonzero decimal exponent.
fn expout<H: Hal>(h: &mut H) {
    let v = g1(0o157);
    if v == 0 {
        return;
    }
    echo(h, b'E');
    let sign = if neg(v) {
        s1(0o157, 0u8.wrapping_sub(v));
        b'-'
    } else {
        b'+'
    };
    echo(h, sign);
    let mut tens = 0u8;
    loop {
        let a = g1(0o157).wrapping_sub(10);
        if neg(a) {
            break;
        }
        s1(0o157, a);
        tens += 1;
    }
    echo(h, b'0' + tens);
    echo(h, g1(0o157).wrapping_add(b'0'));
}

/// FPOUT: print the FPACC. The flag at 110 asks for fixed point; numbers
/// that do not fit print as 0.ddddddd with an exponent.
pub fn fpout<H: Hal>(h: &mut H) {
    s1(0o157, 0);
    let sign = if neg(g1(0o126)) {
        complm(0o124, 3);
        b'-'
    } else {
        b' '
    };
    echo(h, sign);
    let mut float = g1(0o110) == 0;
    if !float {
        let e = g1(0o127);
        float = neg(e) || neg(23u8.wrapping_sub(e));
    }
    if float {
        s1(0o110, 0);
        echo(h, b'0');
        echo(h, b'.');
    }
    // OUTFIX, DECEXT: scale into [2^-4, 1) by tenths and tens, counting
    // the decimal exponent at 157.
    let mut e = g1(0o127).wrapping_add(0xFF);
    s1(0o127, e);
    loop {
        if !neg(e) {
            facxop(at(PG1, 0o214));
            fpmult();
            inc1(0o157);
        } else {
            if !neg(e.wrapping_add(4)) {
                break;
            }
            facxop(at(PG1, 0o210));
            fpmult();
            dec1(0o157);
        }
        e = g1(0o127);
    }
    // DECOUT
    move1(0o124, 0o164, 3);
    s1(0o167, 0);
    rotl(0o164, 3, false);
    outx10();
    while inc1(0o127) != 0 {
        rotr(0o167, 4, false);
    }
    // OUTDIG
    s1(0o107, 7);
    let mut digit = true;
    if g1(0o167) == 0 {
        // ZERODG: a leading zero is not printed.
        dec1(0o157);
        if g1(0o166) | g1(0o165) | g1(0o164) == 0 {
            s1(0o157, 0);
        }
        digit = false;
    }
    loop {
        if digit {
            let a = g1(0o167);
            if a == 0 && g1(0o110) != 0 && neg(g1(0o157)) && g1(0o166) & 0o340 == 0 {
                return;
            }
            echo(h, a.wrapping_add(b'0'));
        }
        digit = true;
        // DECRDG
        if g1(0o110) != 0 {
            if dec1(0o157) == 0 {
                echo(h, b'.');
            }
            if dec1(0o107) == 0 {
                return;
            }
        } else if dec1(0o107) == 0 {
            return expout(h);
        }
        outx10();
    }
}
