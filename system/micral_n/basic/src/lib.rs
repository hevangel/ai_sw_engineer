//! SCELBAL, the 1974-75 SCELBI BASIC for the 8008, reimplemented in Rust
//! (spec/spec.md).
//!
//! `no_std`, no allocation; all I/O goes through [`Hal`]. The interpreter
//! follows the original routine by routine (the names in the comments are
//! the listing's labels) over the original's memory layout ([`mem`]): its
//! behaviour, error codes, number formats and quirks are the 1975 program's,
//! which the oracle transcripts check byte for byte.
//!
//! An error does what the original's does: report and jump to the executive
//! from wherever it happened ([`fail`]), abandoning the calls in progress.
//! On the 8008 that is safe and cheap, since the return address stack is a
//! circular register file; the target jumps to `basic_restart` (asm/start.s)
//! and the host unwinds with a panic that its caller catches (spec "Target").
//! Within a run, [`Go`] says where the executive goes next, as the listing's
//! `JMP EXEC` / `EXEC1` / `NXTLIN` / `SAMLIN` / `DIRECT` do.
//!
//! Written for the 8008's 7-level stack: the statements are inlined into
//! the executive, and the parser, operators and functions into `eval`, so
//! the deepest chain (an array subscript that takes a square root) stays
//! within seven calls.
#![no_std]

pub mod fp;
pub mod hal;
pub mod mem;

pub use hal::Hal;

use core::cell::UnsafeCell;
use fp::{
    cfalse, dinput, facxop, fload, fpadd, fpcomp, fpdiv, fpfix, fpflt, fpmult, fpone, fpout, fpsub,
    fstore, opload,
};
use mem::{
    add16, at, g1, g26, g27, inl, moveit, rd, s1, s26, s27, sub16, wr, BGNPGRAM, ENDPGRAM, PG1,
    PG26, PG27, PG57,
};

/// The error being reported across a restart: two letters, 0 for none.
struct Pending(UnsafeCell<[u8; 2]>);
// One thread; written by `fail`, read by `exec`.
unsafe impl Sync for Pending {}
static PENDING: Pending = Pending(UnsafeCell::new([0; 2]));

/// ERROR: report a two-letter error at the executive.
#[inline(never)]
pub fn fail(a: u8, c: u8) -> ! {
    unsafe { *PENDING.0.get() = [a, c] };
    restart()
}

#[cfg(target_os = "none")]
#[inline(always)]
fn restart() -> ! {
    extern "C" {
        /// asm/start.s: jumps to the executive.
        fn basic_restart() -> !;
    }
    unsafe { basic_restart() }
}

#[cfg(not(target_os = "none"))]
#[inline(always)]
fn restart() -> ! {
    panic!("SCELBAL restart")
}

#[inline(always)]
fn bigerr() -> ! {
    fail(b'B', b'G')
}

#[inline(always)]
fn synerr() -> ! {
    fail(b'S', b'Y')
}

/// Where the executive continues.
enum Go {
    /// Print READY, then read a line.
    Exec,
    /// Read a line.
    Exec1,
    /// The next program line.
    NxtLin,
    /// The program line at the line pointer.
    SamLin,
    /// The statement in the line buffer (its token is at 26 203).
    Direct,
}

// Characters as SCELBAL holds them, bit 7 set.
const SP: u8 = 0o240;
const DQUOTE: u8 = 0o242;
const DOLLAR: u8 = 0o244;
const SQUOTE: u8 = 0o247;
const LPAREN: u8 = 0o250;
const RPAREN: u8 = 0o251;
const COMMA: u8 = 0o254;
const SEMI: u8 = 0o273;
const EQ: u8 = 0o275;
const CR: u8 = 0o215;
const LF: u8 = 0o212;

/// The sign bit (the 8008's S flag after a load or arithmetic).
#[inline(always)]
pub fn neg(v: u8) -> bool {
    v & 0x80 != 0
}

/// `CPI b` then `JTS`: a - b is negative.
#[inline(always)]
fn lt(a: u8, b: u8) -> bool {
    neg(a.wrapping_sub(b))
}

/// The listing's digit test: `CPI 260; JTS no; CPI 272; JFS no`.
fn digit(c: u8) -> bool {
    !lt(c, 0o260) && lt(c, 0o272)
}

/// ECHO: print a character and count the column (01 043).
#[inline(never)]
pub fn echo<H: Hal>(h: &mut H, c: u8) {
    s1(0o043, g1(0o043).wrapping_add(1));
    h.putc(c & 0x7F);
}

/// CINPUT: a key, in mark parity, echoed as the SCELBI input routine does
/// (ASSUMPTION (spec BAS-1): the Micral N teletype in place of the SCELBI
/// serial routines).
fn cinput<H: Hal>(h: &mut H) -> u8 {
    let c = h.getc() | 0x80;
    h.putc(c & 0x7F);
    c
}

/// CRLF: new line, column 1.
fn crlf<H: Hal>(h: &mut H) {
    echo(h, CR);
    echo(h, LF);
    s1(0o043, 1);
}

/// `n` spaces (0 = 256).
fn spaces<H: Hal>(h: &mut H, mut n: u8) {
    loop {
        echo(h, SP);
        n = n.wrapping_sub(1);
        if n == 0 {
            return;
        }
    }
}

/// TEXTC: print the counted string at `hl`; returns the pointer to its last
/// character, as the routine leaves H:L.
fn textc<H: Hal>(h: &mut H, mut hl: u16) -> u16 {
    let mut n = rd(hl);
    if n == 0 {
        return hl;
    }
    loop {
        hl = hl.wrapping_add(1);
        echo(h, rd(hl));
        n -= 1;
        if n == 0 {
            return hl;
        }
    }
}

/// STRIN: read a line into the counted buffer at `base`, with rubout and
/// control-C, then CR LF.
fn strin<H: Hal>(h: &mut H, base: u16) {
    let mut hl = base;
    let mut n: u8 = 0;
    loop {
        let c = cinput(h);
        if c == 0o377 {
            echo(h, 0o334);
            n = n.wrapping_sub(1);
            if neg(n) {
                n = 0;
            } else {
                hl = hl.wrapping_sub(1);
            }
            continue;
        }
        if c == 0o203 {
            fail(b'^', b'C');
        }
        if c == CR {
            break;
        }
        if c == LF {
            continue;
        }
        hl = hl.wrapping_add(1);
        n += 1;
        wr(hl, c);
        if !lt(n, 0o120) {
            bigerr();
        }
    }
    wr(sub16(hl, n), n);
    crlf(h);
}

/// GETCHR: the line buffer character at the pointer kept at 26 `loc`.
fn getchr(loc: u8) -> u8 {
    let p = g26(loc);
    if !lt(p, 0o120) {
        bigerr();
    }
    g26(p)
}

/// The pointer at 26 `dst` = the one at 26 `src` plus one (`LBM; INB;
/// LLI dst; LMB`): where a scan starts after the previous one.
#[inline(never)]
fn next_ptr(dst: u8, src: u8) {
    s26(dst, g26(src).wrapping_add(1))
}

/// LOOP: advance the pointer at 26 `loc`; true if it was at the end of the
/// line.
fn lp(loc: u8) -> bool {
    let b = g26(loc);
    s26(loc, b.wrapping_add(1));
    g26(0) == b
}

/// CONCT1: append `c` to the counted string at `base`.
fn concat(base: u16, c: u8) {
    let n = rd(base).wrapping_add(1);
    wr(base, n);
    wr(add16(base, n), c);
}

/// CONCTS: append to the SYMBOL BUFFER (26 120).
fn concts(c: u8) {
    concat(at(PG26, 0o120), c)
}

/// CLESYM: empty the SYMBOL BUFFER.
fn clesym() {
    s26(0o120, 0)
}

/// STRCPC: compare `n` characters (0 = 256); the first difference a - b.
fn cmp_n(mut a: u16, mut b: u16, mut n: u8) -> u8 {
    loop {
        let d = rd(a).wrapping_sub(rd(b));
        if d != 0 {
            return d;
        }
        n = n.wrapping_sub(1);
        if n == 0 {
            return 0;
        }
        a = a.wrapping_add(1);
        b = b.wrapping_add(1);
    }
}

/// STRCP: compare counted strings, length first; 0 if equal.
fn strcp(a: u16, b: u16) -> u8 {
    let n = rd(a);
    let d = n.wrapping_sub(rd(b));
    if d != 0 {
        return d;
    }
    cmp_n(a.wrapping_add(1), b.wrapping_add(1), n)
}

/// INSTR: where the counted string at `test` starts in the line buffer,
/// or 0.
fn instr(test: u16) -> u8 {
    let mut de = at(PG26, 0);
    loop {
        de = de.wrapping_add(1);
        if cmp_n(test.wrapping_add(1), de, rd(test)) == 0 {
            return de as u8;
        }
        if g26(0) == de as u8 {
            return 0;
        }
    }
}

/// MOVECP: copy a counted string (count included) within a page.
fn movecp(src: u16, dst: u16) {
    moveit(src, dst, rd(src).wrapping_add(1))
}

/// SAVESY: SYMBOL BUFFER to AUX SYMBOL BUFFER (26 144).
fn savesy() {
    movecp(at(PG26, 0o120), at(PG26, 0o144))
}

/// RESTSY: AUX SYMBOL BUFFER back to the SYMBOL BUFFER.
fn restsy() {
    movecp(at(PG26, 0o144), at(PG26, 0o120))
}

/// MOVEC: copy a counted string across pages (program lines).
fn movec(mut src: u16, mut dst: u16) {
    let mut n = rd(src).wrapping_add(1);
    loop {
        wr(dst, rd(src));
        src = src.wrapping_add(1);
        dst = dst.wrapping_add(1);
        n = n.wrapping_sub(1);
        if n == 0 {
            return;
        }
    }
}

/// The user program line pointer (26 360/361).
fn progptr() -> u16 {
    (g26(0o360) as u16) << 8 | g26(0o361) as u16
}

fn set_progptr(p: u16) {
    s26(0o360, (p >> 8) as u8);
    s26(0o361, p as u8);
}

/// The end of program pointer (26 364/365).
fn endptr() -> u16 {
    (g26(0o364) as u16) << 8 | g26(0o365) as u16
}

/// GETCHP: the character of the current program line at the offset kept
/// at 26 `loc`.
fn getchp(loc: u8) -> u8 {
    rd(add16(progptr(), g26(loc)))
}

/// TABADR: entry `b` of a four-byte table whose entry 0 is at `base`.
fn tabadr(base: u16, b: u8) -> u16 {
    add16(base, b.rotate_left(2))
}

/// The digits of a program line's number into the counted string at
/// `dst`, from line offset 1; stops at the first other character. Uses the
/// offset counter at 26 `loc` (GETAU0 / GOTO5).
fn line_number(loc: u8, dst: u16) {
    s26(loc, 1);
    loop {
        let c = getchp(loc);
        if c != SP {
            if !digit(c) {
                return;
            }
            concat(dst, c);
        }
        let b = g26(loc);
        s26(loc, b.wrapping_add(1));
        if rd(progptr()) == b {
            return;
        }
    }
}

// ---------------------------------------------------------------- SYNTAX

/// SYNTAX: strip the line number into 26 340 and find the statement's
/// token (26 203): 1-12 keywords, 015 implied LET, 016 implied array LET,
/// 0 line number only, 377 none.
fn syntax() {
    clesym();
    s26(0o340, 0);
    s26(0o201, 1);
    loop {
        let c = getchr(0o201);
        if c != SP {
            if !digit(c) {
                s26(0o202, g26(0o201));
                return syntx4();
            }
            concat(at(PG26, 0o340), c);
        }
        if lp(0o201) {
            s26(0o203, 0);
            return;
        }
    }
}

/// SYNTX4: gather characters from the SCAN pointer (26 202) until they
/// spell a keyword.
fn syntx4() {
    loop {
        let c = getchr(0o202);
        if c != SP {
            if c == EQ {
                return s26(0o203, 0o15);
            }
            if c == LPAREN {
                return s26(0o203, 0o16);
            }
            concts(c);
            s26(0o203, 1);
            let mut kw = at(PG27, 0);
            loop {
                if strcp(kw, at(PG26, 0o120)) == 0 {
                    return;
                }
                // SYNTXL: on to the next entry (the table is contiguous).
                kw = at(PG27, (kw as u8).wrapping_add(rd(kw)).wrapping_add(1));
                let t = g26(0o203).wrapping_add(1);
                s26(0o203, t);
                if t == 0o15 {
                    break;
                }
            }
        }
        if lp(0o202) {
            return s26(0o203, 0o377);
        }
    }
}

// ------------------------------------------------------------ expressions

/// Push the FPACC on the ARITHMETIC STACK (01 227 pointer).
fn push_fpacc() {
    let a = g1(0o227).wrapping_add(4);
    s1(0o227, a);
    fstore(at(PG1, a));
}

/// The value slot of the variable named in the SYMBOL BUFFER (page 27
/// offset), adding the name with value 0 if new (LOOKUP / STOSYM).
fn var_slot() -> u8 {
    s26(0o370, 0);
    if g26(0o120) == 1 {
        s26(0o122, 0);
    }
    let mut e: u8 = 0o210;
    loop {
        let v = e.wrapping_add(2);
        if g26(0o121) == g27(e) && g26(0o122) == g27(e.wrapping_add(1)) {
            return v;
        }
        e = v.wrapping_add(4);
        let k = g26(0o370).wrapping_add(1);
        s26(0o370, k);
        if k == g27(0o077) {
            let n = g27(0o077).wrapping_add(1);
            s27(0o077, n);
            if !lt(n, 0o25) {
                bigerr();
            }
            s27(e, g26(0o121));
            s27(e.wrapping_add(1), g26(0o122));
            let v = e.wrapping_add(2);
            let mut i = 0;
            while i < 4 {
                s27(v.wrapping_add(i), 0);
                i += 1;
            }
            return v;
        }
    }
}

/// STOSYM: store the FPACC in the array element set up by ARRAY, or in
/// the variable named in the SYMBOL BUFFER.
fn stosym() {
    if g27(0o201) != 0 {
        s27(0o201, 0);
        return fstore(at(PG57, g27(0o204)));
    }
    let v = var_slot();
    fstore(at(PG27, v));
    clesym();
}

/// FAERR
#[inline(always)]
fn faerr() -> ! {
    s26(0o230, 0);
    fail(b'F', b'A')
}

/// PARNER: unbalanced parentheses.
#[inline(always)]
fn parner() -> ! {
    s26(0o230, 0);
    fail(b'I', b'(')
}

/// FUNARR, at "(": a function name or array name before it goes on the
/// F/A STACK (26 230) as a token: 1-8 functions, -n array n.
fn funarr() {
    if g26(0o120) == 0 {
        return;
    }
    s27(0o202, 0);
    loop {
        let b = g27(0o202).wrapping_add(1);
        s27(0o202, b);
        if strcp(tabadr(at(PG26, 0o274), b), at(PG26, 0o120)) == 0 {
            return funar4();
        }
        if b == 8 {
            break;
        }
    }
    s27(0o202, 0);
    loop {
        let b = g27(0o202).wrapping_add(1);
        s27(0o202, b);
        if strcp(tabadr(at(PG27, 0o114), b), at(PG26, 0o120)) == 0 {
            s27(0o202, 0u8.wrapping_sub(b));
            return funar4();
        }
        if b == g27(0o075) {
            faerr();
        }
    }
}

fn funar4() {
    s26(0o230u8.wrapping_add(g26(0o230)), g27(0o202));
    clesym();
}

/// FPOPER: apply operator `op` to the top of the ARITHMETIC STACK (left)
/// and the FPACC (right).
#[inline(always)]
fn fpoper(op: u8) {
    s26(0o371, op);
    opload(at(PG1, g1(0o227)));
    s1(0o227, g1(0o227).wrapping_sub(4));
    let t = match op {
        1 => return fpadd(),
        2 => return fpsub(),
        3 => return fpmult(),
        4 => return fpdiv(),
        5 => return intexp(),
        0o11..=0o16 => {
            fpsub();
            let a = g1(0o126);
            match op {
                0o11 => neg(a),
                0o12 => a == 0,
                0o13 => a != 0 && !neg(a),
                0o14 => a == 0 || neg(a),
                0o15 => !neg(a),
                _ => a != 0,
            }
        }
        _ => parner(),
    };
    if t {
        fpone()
    } else {
        cfalse()
    }
}

/// PARSER: turn the symbol just ended into a value, then apply the
/// operators on the OPERATOR STACK (26 210) that the new operator (token
/// at 26 176) does not outrank, and push it.
#[inline(always)]
fn parser() {
    let n = g26(0o120);
    if n != 0 {
        let c = g26(0o121);
        if c == 0o256 || digit(c) {
            if n != 1 && g26(0o120u8.wrapping_add(n)) == 0o305 {
                // "1E" before a sign: the sign belongs to the exponent.
                return concts(getchr(0o200));
            }
            push_fpacc();
            dinput(at(PG26, 0o120));
        } else {
            let v = var_slot();
            push_fpacc();
            fload(at(PG27, v));
        }
    }
    loop {
        // PARSE (PARSE2 for ")", which applies everything back to its "(").
        clesym();
        let t = g26(0o176);
        let top = g26(0o210u8.wrapping_add(g26(0o210)));
        if t == 7 {
            if top == 0 {
                parner();
            }
        } else {
            let b = g26(0o240u8.wrapping_add(t));
            let m = g26(top.wrapping_add(0o257));
            if !(b == m || lt(b, m)) {
                let p = g26(0o210).wrapping_add(1);
                s26(0o210, p);
                return s26(0o210u8.wrapping_add(p), t);
            }
            if top == 0 {
                return;
            }
        }
        s26(0o210, g26(0o210).wrapping_sub(1));
        if t == 7 && top == 6 {
            return;
        }
        fpoper(top);
    }
}

/// INTEXP: FPOP ^ FPACC by repeated multiplication (or division for a
/// negative power) by the integer part of the power.
fn intexp() {
    let a = g1(0o126);
    s1(0o003, a);
    if a == 0 {
        return fpone();
    }
    if neg(a) {
        fpcomp();
    }
    fpfix();
    s1(0o013, g1(0o124));
    moveit(at(PG1, 0o134), at(PG1, 0o014), 4);
    fpone();
    let divide = neg(g1(0o003));
    loop {
        facxop(at(PG1, 0o014));
        if divide {
            fpdiv();
        } else {
            fpmult();
        }
        let c = g1(0o013).wrapping_sub(1);
        s1(0o013, c);
        if c == 0 {
            return;
        }
    }
}

/// INTX: INT, the largest integer not above.
fn intx() {
    if neg(g1(0o126)) {
        fstore(at(PG1, 0o014));
        int1();
        opload(at(PG1, 0o014));
        fpsub();
        let whole = g1(0o126) == 0;
        fload(at(PG1, 0o014));
        if whole {
            return;
        }
        facxop(at(PG1, 0o024));
        fpadd();
    }
    int1()
}

fn int1() {
    fpfix();
    s1(0o123, 0);
    fpflt();
}

/// SQRX: Newton's iteration from 1.0 on the mantissa, half the exponent
/// added back.
fn sqrx() {
    fstore(at(PG1, 0o014));
    let a = g1(0o126);
    if neg(a) {
        fail(b'S', b'Q');
    }
    if a == 0 {
        return cfalse();
    }
    let e = g1(0o017);
    let half = if !neg(e) {
        s1(0o017, e & 1);
        e >> 1
    } else {
        let m = 0u8.wrapping_sub(e);
        let rem = m & 1;
        s1(0o017, rem);
        0u8.wrapping_sub((m >> 1).wrapping_add(rem))
    };
    s1(0o013, half);
    moveit(at(PG1, 0o004), at(PG1, 0o034), 4);
    cfalse();
    fstore(at(PG1, 0o044));
    loop {
        fload(at(PG1, 0o034));
        opload(at(PG1, 0o014));
        fpdiv();
        opload(at(PG1, 0o034));
        fpadd();
        s1(0o127, g1(0o127).wrapping_sub(1));
        fstore(at(PG1, 0o034));
        opload(at(PG1, 0o044));
        fpsub();
        // PATCH 2: converged below 2^-9, or no change at all.
        if lt(g1(0o127), 0o367) || g1(0o126) == 0 {
            break;
        }
        moveit(at(PG1, 0o034), at(PG1, 0o044), 4);
    }
    s1(0o037, g1(0o037).wrapping_add(g1(0o013)));
    fload(at(PG1, 0o034));
}

/// RNDX: seed = (seed * A + C) mod 65536; the result is seed / 65536.
fn rndx() {
    fload(at(PG1, 0o064));
    opload(at(PG1, 0o050));
    fpmult();
    opload(at(PG1, 0o060));
    fpadd();
    fstore(at(PG1, 0o064));
    s1(0o127, g1(0o127).wrapping_sub(16));
    fpfix();
    s1(0o123, 0);
    s1(0o127, 0);
    fpflt();
    s1(0o127, g1(0o127).wrapping_add(16));
    opload(at(PG1, 0o064));
    fpsub();
    fstore(at(PG1, 0o064));
    s1(0o127, g1(0o127).wrapping_sub(16));
}

/// TAB1: space (or return and space) to the column in the FPACC LSW.
fn tab1<H: Hal>(h: &mut H) {
    loop {
        let a = g1(0o124).wrapping_sub(g1(0o043));
        s26(0o177, 0o377);
        if !neg(a) {
            if a != 0 {
                spaces(h, a);
            }
            return;
        }
        // BACKSP
        echo(h, CR);
        echo(h, CR);
        s1(0o043, 1);
        let t = g1(0o124);
        if neg(t) || t == 0 {
            return;
        }
    }
}

/// PRIGHT, at ")": apply the function or array on the F/A STACK.
#[inline(always)]
fn pright<H: Hal>(h: &mut H) {
    let o = 0o230u8.wrapping_add(g26(0o230));
    let t = g26(o);
    s26(o, 0);
    s27(0o203, t);
    if t == 0 {
        return;
    }
    if neg(t) {
        // PRIGH1: array element.
        if neg(g1(0o126)) {
            fail(b'O', b'R');
        }
        fpfix();
        let c = g1(0o124).wrapping_sub(1).rotate_left(2);
        let e = (!t).rotate_left(2).wrapping_add(0o122);
        return fload(at(PG57, g27(e).wrapping_add(c)));
    }
    match t {
        1 => intx(),
        2 => {
            let a = g1(0o126);
            if a != 0 {
                if neg(a) {
                    fload(at(PG1, 0o024))
                } else {
                    fpone()
                }
            }
        }
        3 => {
            if neg(g1(0o126)) {
                fpcomp()
            }
        }
        4 => sqrx(),
        5 => {
            fpfix();
            tab1(h);
        }
        6 => rndx(),
        7 => {
            fpfix();
            echo(h, g1(0o124));
            s26(0o177, 0o377);
        }
        // UDF, and the listing's safety halt for any other token.
        _ => loop {
            h.halt()
        },
    }
}

/// EVAL: evaluate the line buffer from the EVAL pointer (26 276) to EVAL
/// FINISH (26 277) into the FPACC.
#[inline(never)]
fn eval<H: Hal>(h: &mut H) {
    s1(0o227, 0o224);
    s26(0o230, 0);
    clesym();
    s26(0o210, 0);
    s26(0o200, g26(0o276));
    // After the last character, PARSEP: PARSER once more with token 0.
    let mut last = false;
    loop {
        let c = if last { SP } else { getchr(0o200) };
        let op = match c {
            SP => 0,
            0o253 => 1,
            0o255 => {
                if g26(0o120) == 0 {
                    let t = g26(0o176);
                    if t != 7 {
                        if t == 3 || t == 5 {
                            synerr();
                        }
                        // Unary minus: 0 - x.
                        s26(0o120, 1);
                        s26(0o121, 0o260);
                    }
                }
                2
            }
            0o252 => 3,
            0o257 => 4,
            LPAREN => {
                next_ptr(0o230, 0o230);
                funarr();
                6
            }
            RPAREN => 7,
            0o336 => 5,
            0o274 | EQ | 0o276 => {
                // < = > alone or paired: <= =< >= => <> ><
                let p = g26(0o200);
                s26(0o200, p.wrapping_add(1));
                let d = getchr(0o200);
                let pair = match (c, d) {
                    (0o274, EQ) | (EQ, 0o274) => 0o14,
                    (EQ, 0o276) | (0o276, EQ) => 0o15,
                    (0o274, 0o276) | (0o276, 0o274) => 0o16,
                    _ => 0,
                };
                if pair != 0 {
                    pair
                } else {
                    s26(0o200, p);
                    c - 0o274 + 0o11
                }
            }
            _ => {
                concts(c);
                0
            }
        };
        if op != 0 || last {
            s26(0o176, op);
            parser();
            if op == 7 {
                pright(h);
                s26(0o230, g26(0o230).wrapping_sub(1));
            }
        }
        if last {
            break;
        }
        let b = g26(0o200);
        s26(0o200, b.wrapping_add(1));
        last = g26(0o277) == b;
    }
    if g1(0o227) != 0o230 {
        synerr();
    }
}

/// ARRAY / ARRAY1 / ARRAY2: set up the array element whose "(" is at the
/// line offset kept at 26 `loc` for STOSYM.
#[inline(never)]
fn array<H: Hal>(h: &mut H, loc: u8) {
    let b = g26(loc).wrapping_add(1);
    s26(0o276, b);
    s26(0o206, b);
    while getchr(0o206) != RPAREN {
        if lp(0o206) {
            fail(b'A', b'F');
        }
    }
    s26(0o277, g26(0o206).wrapping_sub(1));
    s26(0o207, 0);
    loop {
        let k = g26(0o207).wrapping_add(1);
        s26(0o207, k);
        if strcp(tabadr(at(PG27, 0o114), k), at(PG26, 0o120)) == 0 {
            break;
        }
        if k == g27(0o075) {
            faerr();
        }
    }
    eval(h);
    fpfix();
    let base = rd(inl(inl(tabadr(at(PG27, 0o114), g26(0o207)))));
    let a = g1(0o124).wrapping_sub(1).rotate_left(2).wrapping_add(base);
    s27(0o204, a);
    s27(0o201, 0o377);
}

// ------------------------------------------------------------- statements

/// PRINT: fields separated by "," (next 16-column tab) or ";", quoted
/// text in ' or ".
#[inline(always)]
fn print<H: Hal>(h: &mut H) -> Go {
    if !lt(g26(0o202), g26(0)) {
        crlf(h);
        return Go::NxtLin;
    }
    loop {
        // PRINT1
        clesym();
        next_ptr(0o203, 0o202);
        let quote = loop {
            let c = getchr(0o203);
            if c == SQUOTE || c == DQUOTE {
                break c;
            }
            if c == COMMA || c == SEMI || lp(0o203) {
                break 0;
            }
        };
        if quote != 0 {
            s26(0o367, quote);
            clesym();
            next_ptr(0o204, 0o203);
            loop {
                let c = getchr(0o204);
                if c == g26(0o367) {
                    break;
                }
                echo(h, c);
                if lp(0o204) {
                    s26(0o367, 0);
                    fail(b'I', b'Q');
                }
            }
            let b = g26(0o204);
            s26(0o202, b);
            if b != g26(0) {
                continue;
            }
            crlf(h);
            s26(0o367, 0);
            return Go::NxtLin;
        }
        // PRINT3
        next_ptr(0o276, 0o202);
        s26(0o277, g26(0o203).wrapping_sub(1));
        if g26(0o367) != 0 {
            s26(0o367, 0);
        } else {
            eval(h);
            let tab = g26(0o177);
            s1(0o110, 0o377);
            if tab == 0 {
                // PFPOUT
                if g1(0o126) == 0 {
                    echo(h, SP);
                    echo(h, 0o260);
                } else {
                    if g1(0o127) == 0 {
                        s1(0o110, 0);
                    }
                    fpout(h);
                }
            }
            s26(0o177, 0);
        }
        // PRINT6
        if getchr(0o203) == COMMA && !neg(g26(0).wrapping_sub(g26(0o203))) {
            // PCOMMA
            let col = g1(0o043);
            spaces(h, (col & 0o360).wrapping_add(0o20).wrapping_sub(col));
        }
        let b = g26(0o203);
        s26(0o202, b);
        if lt(b, g26(0)) {
            continue;
        }
        let c = getchr(0);
        if c != COMMA && c != SEMI {
            crlf(h);
        }
        return Go::NxtLin;
    }
}

/// LET (token 3), implied LET (LET0, token 015) and implied array LET
/// (token 016): find the variable, evaluate after "=" and store.
#[inline(always)]
fn let_<H: Hal>(h: &mut H, token: u8) -> Go {
    if token == 0o15 {
        // LET0: the name is in the SYMBOL BUFFER, SCAN is at "=".
        savesy();
        s26(0o203, g26(0o202));
    } else {
        if token == 3 {
            clesym();
            s26(0o144, 0);
        } else {
            array(h, 0o202);
            s26(0o202, g26(0o206));
            savesy();
        }
        // LET1: the name into the AUX SYMBOL BUFFER up to "=".
        next_ptr(0o203, 0o202);
        loop {
            let c = getchr(0o203);
            if c != SP {
                if c == EQ {
                    break;
                }
                if c == LPAREN {
                    restsy();
                    array(h, 0o203);
                    s26(0o203, g26(0o206));
                } else {
                    concat(at(PG26, 0o144), c);
                }
            }
            if lp(0o203) {
                fail(b'L', b'E');
            }
        }
    }
    // LET5
    next_ptr(0o276, 0o203);
    s26(0o277, g26(0));
    eval(h);
    restsy();
    stosym();
    Go::NxtLin
}

/// The digits after the keyword into the AUX LINE NR BUFFER (26 350), then
/// find that line (GOTO).
#[inline(never)]
fn goto() -> Go {
    s26(0o350, 0);
    next_ptr(0o203, 0o202);
    loop {
        let c = getchr(0o203);
        if c != SP {
            if !digit(c) {
                break;
            }
            concat(at(PG26, 0o350), c);
        }
        if lp(0o203) {
            break;
        }
    }
    set_progptr(at(BGNPGRAM, 0));
    loop {
        clesym();
        line_number(0o204, at(PG26, 0o120));
        if strcp(at(PG26, 0o120), at(PG26, 0o350)) == 0 {
            return Go::SamLin;
        }
        let p = progptr();
        let p = add16(p, rd(p).wrapping_add(1));
        set_progptr(p);
        if p == endptr() {
            fail(b'U', b'N');
        }
    }
}

/// IF ... THEN / GOTO: a line number after it is a GOTO, anything else a
/// statement.
#[inline(always)]
fn if_<H: Hal>(h: &mut H) -> Go {
    next_ptr(0o276, 0o202);
    clesym();
    let mut e = instr(at(PG1, 0o320));
    if e == 0 {
        e = instr(at(PG27, 0o013));
        if e == 0 {
            fail(b'I', b'F');
        }
    }
    s26(0o277, e.wrapping_sub(1));
    eval(h);
    if g1(0o126) == 0 {
        return Go::NxtLin;
    }
    let a = g26(0o277).wrapping_add(5);
    s26(0o202, a);
    s26(0o204, a.wrapping_add(1));
    let c = loop {
        let c = getchr(0o204);
        if c != SP {
            break c;
        }
        if lp(0o204) {
            fail(b'I', b'F');
        }
    };
    if digit(c) {
        return goto();
    }
    // IF4: the statement moves to the front of the line buffer.
    let p = g26(0o204);
    let n = g26(0).wrapping_sub(p).wrapping_add(1);
    s26(0, n);
    moveit(at(PG26, p), at(PG26, 1), n);
    s26(0o202, 1);
    syntx4();
    Go::Direct
}

/// GOSUB: push the line pointer (page 0 for a direct statement) on the
/// GOSUB STACK (27 073 pointer), then GOTO.
#[inline(always)]
fn gosub() -> Go {
    let d = if g26(0o340) == 0 { 0 } else { g26(0o360) };
    let sp = g27(0o073).wrapping_add(2);
    if !lt(sp, 0o21) {
        fail(b'G', b'S');
    }
    s27(0o073, sp);
    let l = 0o076u8.wrapping_add(sp);
    s27(l, d);
    s27(l.wrapping_add(1), g26(0o361));
    goto()
}

#[inline(always)]
fn return_() -> Go {
    let sp = g27(0o073).wrapping_sub(2);
    if neg(sp) {
        fail(b'R', b'T');
    }
    s27(0o073, sp);
    let l = sp.wrapping_add(0o100);
    let d = g27(l);
    if d == 0 {
        return Go::Exec;
    }
    s26(0o360, d);
    s26(0o361, g27(l.wrapping_add(1)));
    Go::NxtLin
}

/// INPUTX: a value for the variable in the SYMBOL BUFFER: one key for a
/// name ending in "$", else "?" and a number.
#[inline(never)]
fn inputx<H: Hal>(h: &mut H) {
    let n = g26(0o120);
    if g26(0o120u8.wrapping_add(n)) == DOLLAR {
        s26(0o120, n.wrapping_sub(1));
        cfalse();
        let c = cinput(h);
        s1(0o124, c);
        fpflt();
    } else {
        echo(h, 0o277);
        strin(h, at(PG26, 0o144));
        dinput(at(PG26, 0o144));
    }
    stosym();
}

#[inline(always)]
fn input<H: Hal>(h: &mut H) -> Go {
    loop {
        clesym();
        next_ptr(0o203, 0o202);
        loop {
            let c = getchr(0o203);
            if c == COMMA {
                inputx(h);
                s26(0o202, g26(0o203));
                break;
            }
            if c == LPAREN {
                array(h, 0o203);
                s26(0o203, g26(0o206));
            } else if c != SP {
                concts(c);
            }
            if lp(0o203) {
                inputx(h);
                return Go::NxtLin;
            }
        }
    }
}

/// The name before "=" into the AUX SYMBOL BUFFER, from the line offset
/// kept at 26 `loc` (FOR2 / NEXT7); the error letters if there is none.
fn name_before_eq(loc: u8, a: u8, c: u8) {
    loop {
        let ch = getchr(loc);
        if ch != SP {
            if ch == EQ {
                return;
            }
            concat(at(PG26, 0o144), ch);
        }
        if lp(loc) {
            fail(a, c);
        }
    }
}

/// FOR: push the line pointer and the variable name on the FOR/NEXT STACK
/// (27 205 pointer, four bytes an entry) and store the initial value.
#[inline(always)]
fn for_<H: Hal>(h: &mut H) -> Go {
    s26(0o144, 0);
    s26(0o146, 0);
    let sp = g27(0o205).wrapping_add(1);
    s27(0o205, sp);
    let l = sp.rotate_left(2).wrapping_add(0o134);
    s27(l, g26(0o360));
    s27(l.wrapping_add(1), g26(0o361));
    let to = instr(at(PG1, 0o325));
    if to == 0 {
        fail(b'F', b'E');
    }
    next_ptr(0o204, 0o202);
    s26(0o203, to);
    name_before_eq(0o204, b'F', b'E');
    next_ptr(0o276, 0o204);
    s26(0o277, g26(0o203).wrapping_sub(1));
    eval(h);
    restsy();
    if g26(0o144) == 1 {
        s26(0o146, 0);
    }
    let l = g27(0o205).rotate_left(2).wrapping_add(0o136);
    moveit(at(PG26, 0o145), at(PG27, l), 2);
    stosym();
    Go::NxtLin
}

#[inline(always)]
fn fornxt() -> ! {
    fail(b'F', b'N')
}

/// NEXT: re-read the FOR line for its limit, step and variable; step the
/// variable and go back after the FOR, or pop and go on.
#[inline(always)]
fn next<H: Hal>(h: &mut H) -> Go {
    s26(0o144, 0);
    next_ptr(0o201, 0o202);
    loop {
        let c = getchr(0o201);
        if c != SP {
            concat(at(PG26, 0o144), c);
        }
        if lp(0o201) {
            break;
        }
    }
    if g26(0o144) == 1 {
        s26(0o146, 0);
    }
    let l = g27(0o205).rotate_left(2).wrapping_add(0o136);
    if cmp_n(at(PG27, l), at(PG26, 0o145), 2) != 0 {
        fornxt();
    }
    s26(0o362, g26(0o360));
    s26(0o363, g26(0o361));
    let l = g27(0o205).rotate_left(2).wrapping_add(0o134);
    s26(0o360, g27(l));
    s26(0o361, g27(l.wrapping_add(1)));
    movec(progptr(), at(PG26, 0));
    let to = instr(at(PG1, 0o325));
    if to == 0 {
        fornxt();
    }
    s26(0o276, to.wrapping_add(2));
    let step = instr(at(PG1, 0o330));
    if step == 0 {
        fpone();
        fstore(at(PG1, 0o304));
        s26(0o277, g26(0));
        eval(h);
        fstore(at(PG1, 0o310));
    } else {
        s26(0o277, step.wrapping_sub(1));
        eval(h);
        fstore(at(PG1, 0o310));
        s26(0o276, g26(0o277).wrapping_add(5));
        s26(0o277, g26(0));
        eval(h);
        fstore(at(PG1, 0o304));
    }
    s26(0o144, 0);
    let f = instr(at(PG27, 0o034));
    s26(0o202, f);
    if f == 0 {
        fornxt();
    }
    s26(0o203, f.wrapping_add(3));
    name_before_eq(0o203, b'F', b'N');
    s26(0o276, g26(0o202).wrapping_add(3));
    s26(0o277, g26(0o203).wrapping_sub(1));
    eval(h);
    facxop(at(PG1, 0o304));
    fpadd();
    fstore(at(PG1, 0o314));
    facxop(at(PG1, 0o310));
    fpsub();
    let s = g1(0o306);
    let a = g1(0o126);
    if s == 0 {
        fornxt();
    }
    let more = if neg(s) { !neg(a) } else { neg(a) || a == 0 };
    if more {
        fload(at(PG1, 0o314));
        restsy();
        stosym();
    } else {
        s26(0o360, g26(0o362));
        s26(0o361, g26(0o363));
        s27(0o205, g27(0o205).wrapping_sub(1));
    }
    Go::NxtLin
}

#[inline(always)]
fn dimerr() -> ! {
    fail(b'D', b'E')
}

/// DIM: name(size), ...: the ARRAY VARIABLES table (27 120, four bytes an
/// entry, count at 27 075) and the start of each array on page 57.
#[inline(always)]
fn dim() -> Go {
    loop {
        clesym();
        next_ptr(0o203, 0o202);
        loop {
            let c = getchr(0o203);
            if c != SP {
                if c == LPAREN {
                    break;
                }
                concts(c);
            }
            if lp(0o203) {
                dimerr();
            }
        }
        s26(0o206, 0);
        let known = loop {
            let l = g26(0o206).rotate_left(2).wrapping_add(0o114);
            if strcp(at(PG27, l), at(PG26, 0o120)) == 0 {
                break true;
            }
            let b = g26(0o206).wrapping_add(1);
            s26(0o206, b);
            if g27(0o075) == b.wrapping_sub(1) {
                break false;
            }
        };
        if !known {
            let b = g27(0o075).wrapping_add(1);
            s27(0o075, b);
            s27(0o076, b);
            s26(0o206, b);
            movec(
                at(PG26, 0o120),
                at(PG27, b.rotate_left(2).wrapping_add(0o114)),
            );
            clesym();
            next_ptr(0o204, 0o203);
            loop {
                let c = getchr(0o204);
                if c != SP {
                    if c == RPAREN {
                        break;
                    }
                    if !digit(c) {
                        dimerr();
                    }
                    concts(c);
                }
                if lp(0o204) {
                    dimerr();
                }
            }
            dinput(at(PG26, 0o120));
            fpfix();
            let size = g1(0o124).rotate_left(2);
            let l = g27(0o076)
                .wrapping_sub(1)
                .rotate_left(2)
                .wrapping_add(0o122);
            let start = g27(l);
            s27(l.wrapping_add(4), start.wrapping_add(size));
            s26(0o203, g26(0o204));
        }
        // DIM9: another array after a comma?
        loop {
            if getchr(0o203) == COMMA {
                s26(0o202, g26(0o203));
                break;
            }
            if lp(0o203) {
                return Go::NxtLin;
            }
        }
    }
}

/// DIRECT: run the statement whose token SYNTAX left at 26 203.
#[inline(always)]
fn direct<H: Hal>(h: &mut H) -> Go {
    match g26(0o203) {
        1 => Go::NxtLin,
        2 => if_(h),
        t @ (3 | 0o15 | 0o16) => let_(h, t),
        4 => goto(),
        5 => print(h),
        6 => input(h),
        7 => for_(h),
        8 => next(h),
        9 => gosub(),
        0o12 => return_(),
        0o13 => dim(),
        0o14 => Go::Exec,
        _ => synerr(),
    }
}

// --------------------------------------------------------------- executive

/// Store the line in the line buffer in the program, by line number:
/// insert, replace, or (a number alone) delete.
#[inline(always)]
fn enter_line() -> Go {
    set_progptr(at(BGNPGRAM, 0));
    loop {
        // GETAUX
        s26(0o350, 0);
        line_number(0o201, at(PG26, 0o350));
        let p = progptr();
        let n = rd(p);
        if n == 0 {
            break;
        }
        let d = strcp(at(PG26, 0o350), at(PG26, 0o340));
        if neg(d) {
            set_progptr(add16(p, n.wrapping_add(1)));
            continue;
        }
        if d == 0 {
            remove(p, n.wrapping_add(1));
            if g26(0o203) == 0 {
                return Go::Exec;
            }
        }
        break;
    }
    // NOSAME
    let p = progptr();
    insert(p, g26(0).wrapping_add(1));
    movec(at(PG26, 0), p);
    Go::Exec1
}

/// REMOVE: delete `n` bytes at `hl` up to the program's end mark.
fn remove(mut hl: u16, n: u8) {
    loop {
        let c = rd(add16(hl, n));
        wr(hl, c);
        if c == 0 {
            break;
        }
        hl = hl.wrapping_add(1);
    }
    let (lo, borrow) = g26(0o365).overflowing_sub(n);
    s26(0o365, lo);
    if borrow {
        s26(0o364, g26(0o364).wrapping_sub(1));
    }
}

/// INSERT: open `n` bytes at `de` (the program must stay below ENDPGRAM),
/// then INCLIN.
fn insert(de: u16, n: u8) {
    let mut hl = endptr();
    if !lt((add16(hl, n) >> 8) as u8, ENDPGRAM) {
        bigerr();
    }
    loop {
        wr(add16(hl, n), rd(hl));
        if hl == de {
            break;
        }
        hl = hl.wrapping_sub(1);
    }
    let e = add16(endptr(), g26(0).wrapping_add(1));
    s26(0o364, (e >> 8) as u8);
    s26(0o365, e as u8);
}

/// SCR: empty the program, variables and arrays.
fn scr() {
    s26(0o364, BGNPGRAM);
    s26(0o365, 0);
    s27(0o077, 1);
    s27(0o075, 0);
    s27(0o120, 0);
    s27(0o210, 0);
    s27(0o211, 0);
    wr(at(BGNPGRAM, 0), 0);
    let mut i: u8 = 0;
    loop {
        wr(at(PG57, i), 0);
        i = i.wrapping_add(1);
        if i == 0 {
            break;
        }
    }
}

/// EXEC1: read a line; run a command, store a numbered line, or run a
/// direct statement.
#[inline(always)]
fn exec1<H: Hal>(h: &mut H) -> Go {
    let line = at(PG26, 0);
    strin(h, line);
    if g26(0) == 0 {
        return Go::Exec1;
    }
    if strcp(at(PG1, 0o335), line) == 0 {
        // LIST
        let mut p = at(BGNPGRAM, 0);
        while rd(p) != 0 {
            p = textc(h, p).wrapping_add(1);
            crlf(h);
        }
        return Go::Exec;
    }
    if strcp(at(PG1, 0o342), line) == 0 {
        // RUN
        s27(0o073, 0);
        s27(0o205, 0);
        set_progptr(at(BGNPGRAM, 0));
        return Go::SamLin;
    }
    if strcp(at(PG1, 0o346), line) == 0 {
        scr();
        return Go::Exec;
    }
    // SAVE and LOAD are the user's to provide; this build has none.
    if strcp(line, at(PG1, 0o272)) == 0 || strcp(at(PG1, 0o277), line) == 0 {
        return Go::Exec;
    }
    set_progptr(at(BGNPGRAM, 0));
    syntax();
    if neg(g26(0o203)) {
        synerr();
    }
    if g26(0o340) == 0 {
        return Go::Direct;
    }
    enter_line()
}

/// Power on: the original's memory image (mem::init).
pub fn power_on() {
    mem::init();
    unsafe { *PENDING.0.get() = [0; 2] };
}

/// The executive (EXEC), entered at power on and after every error: report
/// the error (FINERR, PATCH 3), then READY and on.
#[inline(always)]
pub fn exec<H: Hal>(h: &mut H) -> ! {
    let [a, c] = unsafe { *PENDING.0.get() };
    if a != 0 {
        unsafe { *PENDING.0.get() = [0; 2] };
        echo(h, a);
        echo(h, c);
        if g26(0o340) != 0 {
            textc(h, at(PG1, 0o366));
            textc(h, at(PG26, 0o340));
        }
        crlf(h);
        s27(0o201, 0);
    }
    let mut go = Go::Exec;
    loop {
        go = match go {
            Go::Exec => {
                textc(h, at(PG1, 0o352));
                exec1(h)
            }
            Go::Exec1 => exec1(h),
            Go::NxtLin => {
                let p = progptr();
                set_progptr(add16(p, rd(p).wrapping_add(1)));
                if g26(0o340) == 0 {
                    Go::Exec
                } else {
                    Go::SamLin
                }
            }
            Go::SamLin => {
                movec(progptr(), at(PG26, 0));
                if g26(0) == 0 {
                    Go::Exec
                } else {
                    syntax();
                    Go::Direct
                }
            }
            Go::Direct => direct(h),
        }
    }
}
