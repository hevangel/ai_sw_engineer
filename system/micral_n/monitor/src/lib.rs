//! MIC 01 monitor for the Micral N, reimplemented in Rust (spec/spec.md).
//!
//! `no_std`, no allocation. All machine access goes through [`Hal`].
//!
//! Written for the 8008's size: counters are `u8`, the number being typed and
//! the character that ended it live in statics (a static costs no more than a
//! frame slot on this machine, and passing a tuple back would go through
//! memory), and the shared steps are separate functions.
#![no_std]

pub mod hal;

pub use hal::{Hal, REG_FLAGS};

const CR: u8 = 0x0D;
const LF: u8 = 0x0A;
const DC1: u8 = 0x11; // reader on
const DC2: u8 = 0x12; // punch on
const DC3: u8 = 0x13; // reader off
const DC4: u8 = 0x14; // punch off
const CAL: u8 = 0x46;

/// A command stopped early: the monitor prints `?` (spec "Commands").
pub struct Cancel;
type R = Result<(), Cancel>;

/// The number last read by `read_hex`, its digit count, and the character
/// that ended it.
static mut VAL: u16 = 0;
static mut NDIG: u8 = 0;
static mut TERM: u8 = 0;

/// The holds (spec "G"): bit n of HOLD_ON set = hold n is patched in.
/// They survive a warm entry; the cold entry zeroes RAM.
static mut HOLD_ON: u8 = 0;
static mut HOLD_ADDR: [u16; 2] = [0; 2];
static mut HOLD_SAVE: [[u8; 3]; 2] = [[0; 3]; 2];

// Single-threaded machine; the accessors keep the `unsafe` in one place.
fn val() -> u16 {
    unsafe { VAL }
}
fn term() -> u8 {
    unsafe { TERM }
}
fn ndig() -> u8 {
    unsafe { NDIG }
}

/// Reset the monitor's state, as the cold entry does (host tests).
pub fn reset() {
    unsafe {
        VAL = 0;
        NDIG = 0;
        TERM = 0;
        HOLD_ON = 0;
    }
}

fn hex_value(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'A'..=b'F' => c - (b'A' - 10),
        _ => 0xFF,
    }
}

fn is_sep(c: u8) -> bool {
    c == b' ' || c == b','
}

#[inline(never)]
fn crlf<H: Hal>(h: &mut H) {
    h.putc(CR);
    h.putc(LF);
}

#[inline(never)]
fn nibble<H: Hal>(h: &mut H, v: u8) {
    let v = v & 15;
    h.putc(if v < 10 { b'0' + v } else { b'A' - 10 + v });
}

#[inline(never)]
fn hex8<H: Hal>(h: &mut H, v: u8) {
    nibble(h, v >> 4);
    nibble(h, v);
}

fn hex16<H: Hal>(h: &mut H, v: u16) {
    hex8(h, (v >> 8) as u8);
    hex8(h, v as u8);
}

/// Hex digits, echoed, into VAL until another character arrives (left in
/// TERM, not echoed). An address may have 4 digits (`Err` on a fifth); a
/// byte keeps the last two (§II.3.3).
#[inline(never)]
fn read_hex<H: Hal>(h: &mut H, byte: bool) -> R {
    let mut v: u16 = 0;
    let mut n: u8 = 0;
    loop {
        let c = h.getc();
        let d = hex_value(c);
        if d == 0xFF {
            unsafe {
                VAL = v;
                NDIG = n;
                TERM = c;
            }
            return Ok(());
        }
        h.putc(c);
        n += 1;
        if byte {
            v = ((v as u8) << 4 | d) as u16;
        } else {
            if n > 4 {
                return Err(Cancel);
            }
            v = v << 4 | d as u16;
        }
    }
}

/// An address ended by a separator (echoed).
#[inline(never)]
fn read_addr_sep<H: Hal>(h: &mut H) -> R {
    read_hex(h, false)?;
    let t = term();
    if !is_sep(t) {
        return Err(Cancel);
    }
    h.putc(t);
    Ok(())
}

/// An address ended by CR.
#[inline(never)]
fn read_addr_cr<H: Hal>(h: &mut H) -> R {
    read_hex(h, false)?;
    if term() != CR {
        return Err(Cancel);
    }
    Ok(())
}

/// `first , last CR` into (first, last).
fn read_range<H: Hal>(h: &mut H) -> Result<(u16, u16), Cancel> {
    read_addr_sep(h)?;
    let s = val();
    read_addr_cr(h)?;
    Ok((s, val()))
}

fn expect_cr<H: Hal>(h: &mut H) -> R {
    if h.getc() == CR {
        Ok(())
    } else {
        Err(Cancel)
    }
}

/// Put the original bytes back under every active hold.
fn clear_holds<H: Hal>(h: &mut H) {
    let mut n = 0u8;
    while n < 2 {
        if unsafe { HOLD_ON } & (1 << n) != 0 {
            let a = unsafe { HOLD_ADDR[n as usize] };
            let mut i = 0u8;
            while i < 3 {
                h.poke(a + i as u16, unsafe { HOLD_SAVE[n as usize][i as usize] });
                i += 1;
            }
        }
        n += 1;
    }
    unsafe { HOLD_ON = 0 }
}

/// The program reached hold `n`: restore the holds and say where
/// (ASSUMPTION spec MON-5: CR LF, `*`, the address, CR LF).
pub fn hold_reached<H: Hal>(h: &mut H, n: u8) {
    let a = unsafe { HOLD_ADDR[(n & 1) as usize] };
    clear_holds(h);
    crlf(h);
    h.putc(b'*');
    hex16(h, a);
    crlf(h);
}

/// Read and run one command.
pub fn command<H: Hal>(h: &mut H) {
    let c = h.getc();
    if c == CR || c == LF || c == b' ' || c == 0 {
        return;
    }
    h.putc(c);
    let known = matches!(
        c,
        b'M' | b'A' | b'H' | b'I' | b'T' | b'D' | b'G' | b'C' | b'P' | b'F' | b'N' | b'R'
    );
    if known {
        // ASSUMPTION (spec MON-1): the monitor prints the colon.
        h.putc(b':');
    }
    if !known || run(h, c).is_err() {
        h.putc(b'?');
        crlf(h);
    }
}

fn run<H: Hal>(h: &mut H, c: u8) -> R {
    match c {
        b'M' => {
            // The address ends with CR, space or comma (§II.3.3).
            read_hex(h, false)?;
            let t = term();
            if t == CR {
                crlf(h);
            } else if is_sep(t) {
                h.putc(t);
            } else {
                return Err(Cancel);
            }
            modify(h, val(), MEMORY)
        }
        b'A' => modify(h, 0, REGS),
        b'H' => modify(h, 5, REGS),
        // ASSUMPTION (spec MON-3): I and T are the same command.
        b'I' | b'T' => modify(h, 0, FLAGS_REGS),
        b'D' => {
            let (s, e) = read_range(h)?;
            dump(h, s, e);
            Ok(())
        }
        b'G' => go(h),
        b'C' | b'F' => {
            expect_cr(h)?;
            h.putc(DC2);
            if c == b'F' {
                // ASSUMPTION (spec MON-6): the end block before the trailer.
                h.putc(b':');
                let mut i = 0u8;
                while i < 8 {
                    h.putc(b'0');
                    i += 1;
                }
                crlf(h);
            }
            // ASSUMPTION (spec MON-7): 64 NULs of leader / trailer.
            let mut i = 0u8;
            while i < 64 {
                h.putc(0);
                i += 1;
            }
            h.putc(DC4);
            Ok(())
        }
        b'P' | b'N' => {
            let (s, e) = read_range(h)?;
            crlf(h);
            h.putc(DC2);
            if c == b'P' {
                tape_blocks(h, s, e);
            } else {
                bnpf(h, s, e);
            }
            h.putc(DC4);
            Ok(())
        }
        _ => read_tape(h), // R
    }
}

/// `G:` address [, hold [, hold]] CR (spec "G").
fn go<H: Hal>(h: &mut H) -> R {
    read_hex(h, false)?;
    let start = val();
    let mut holds = [0u16; 2];
    let mut n = 0u8;
    while is_sep(term()) && n < 2 {
        h.putc(term());
        read_hex(h, false)?;
        holds[n as usize] = val();
        n += 1;
    }
    let d = holds[0].wrapping_sub(holds[1]);
    // Holds patch 3 bytes each, so they must be at least 3 apart.
    if term() != CR || (n == 2 && (d < 3 || d > 0xFFFD)) {
        return Err(Cancel);
    }
    crlf(h);
    clear_holds(h);
    let mut i = 0u8;
    while i < n {
        let a = holds[i as usize];
        let entry = h.hold_entry(i);
        let mut k = 0u8;
        while k < 3 {
            let v = h.peek(a + k as u16);
            unsafe { HOLD_SAVE[i as usize][k as usize] = v }
            k += 1;
        }
        h.poke(a, CAL);
        h.poke(a + 1, entry as u8);
        h.poke(a + 2, (entry >> 8) as u8);
        unsafe {
            HOLD_ADDR[i as usize] = a;
            HOLD_ON |= 1 << i;
        }
        i += 1;
    }
    h.go(start);
    Ok(())
}

const MEMORY: u8 = 0;
const REGS: u8 = 1;
const FLAGS_REGS: u8 = 2;

/// Display and modify consecutive bytes: memory from `at`, or the saved
/// registers from index `at` (spec "M, A, H, I").
fn modify<H: Hal>(h: &mut H, mut at: u16, t: u8) -> R {
    // I: the four flags first, one digit each (ASSUMPTION spec MON-4).
    let mut flag: u8 = if t == FLAGS_REGS { 0 } else { 4 };
    loop {
        let flags = h.reg(REG_FLAGS);
        if flag < 4 {
            h.putc(b'0' + ((flags >> flag) & 1));
        } else {
            let v = if t == MEMORY {
                h.peek(at)
            } else {
                if at > 6 {
                    crlf(h);
                    return Ok(());
                }
                h.reg(at as u8)
            };
            // ASSUMPTION (spec MON-2): just the value and the dash.
            hex8(h, v);
        }
        h.putc(b'-');
        read_hex(h, true)?;
        let c = term();
        if c == CR {
            crlf(h);
            return Ok(());
        }
        if !is_sep(c) {
            return Err(Cancel);
        }
        h.putc(c);
        if ndig() != 0 {
            let b = val() as u8;
            if flag < 4 {
                let m = 1 << flag;
                h.set_reg(REG_FLAGS, if b & 1 != 0 { flags | m } else { flags & !m });
            } else if t == MEMORY {
                h.poke(at, b);
            } else {
                h.set_reg(at as u8, b);
            }
        }
        if flag < 4 {
            flag += 1;
        } else {
            at = at.wrapping_add(1);
        }
    }
}

/// Memory dump, 16 columns per line (spec "D").
fn dump<H: Hal>(h: &mut H, start: u16, end: u16) {
    let mut a = start & !0xF;
    loop {
        crlf(h);
        hex16(h, a);
        loop {
            h.putc(b' ');
            if a < start {
                h.putc(b'.');
                h.putc(b'.');
            } else {
                let v = h.peek(a);
                hex8(h, v);
            }
            if a == end {
                crlf(h);
                return;
            }
            a += 1;
            if a as u8 & 15 == 0 {
                break;
            }
        }
        crlf(h);
    }
}

/// Binary blocks of up to 16 bytes (ASSUMPTION spec MON-6).
fn tape_blocks<H: Hal>(h: &mut H, mut a: u16, end: u16) {
    loop {
        let left = end.wrapping_sub(a);
        let n: u8 = if left >= 15 { 16 } else { left as u8 + 1 };
        h.putc(b':');
        hex8(h, n);
        hex16(h, a);
        let mut sum = n.wrapping_add((a >> 8) as u8).wrapping_add(a as u8);
        let mut i = 0u8;
        while i < n {
            let v = h.peek(a);
            sum = sum.wrapping_add(v);
            hex8(h, v);
            a = a.wrapping_add(1);
            i += 1;
        }
        hex8(h, sum.wrapping_neg());
        crlf(h);
        if left < 16 {
            return;
        }
    }
}

/// PROM programming tape, BNPF (ASSUMPTION spec MON-8).
fn bnpf<H: Hal>(h: &mut H, mut a: u16, end: u16) {
    let mut col = 0u8;
    loop {
        let mut v = h.peek(a);
        h.putc(b'B');
        let mut bit = 0u8;
        while bit < 8 {
            h.putc(if v & 0x80 != 0 { b'P' } else { b'N' });
            v <<= 1;
            bit += 1;
        }
        h.putc(b'F');
        col += 1;
        if col == 4 || a == end {
            crlf(h);
            col = 0;
        } else {
            h.putc(b' ');
        }
        if a == end {
            return;
        }
        a += 1;
    }
}

/// Two hex characters from the tape, not echoed, into VAL.
#[inline(never)]
fn tape_byte<H: Hal>(h: &mut H) -> Result<u8, Cancel> {
    let hi = hex_value(h.getc());
    let lo = hex_value(h.getc());
    if (hi | lo) & 0xF0 != 0 {
        return Err(Cancel);
    }
    Ok(hi << 4 | lo)
}

/// `R`: read blocks until the end block (ASSUMPTION spec MON-9).
fn read_tape<H: Hal>(h: &mut H) -> R {
    h.putc(DC1);
    let r = read_blocks(h);
    h.putc(DC3);
    if r.is_ok() {
        crlf(h);
    }
    r
}

fn read_blocks<H: Hal>(h: &mut H) -> R {
    loop {
        while h.getc() != b':' {}
        let n = tape_byte(h)?;
        let ah = tape_byte(h)?;
        let al = tape_byte(h)?;
        let mut a = (ah as u16) << 8 | al as u16;
        let mut sum = n.wrapping_add(ah).wrapping_add(al);
        let mut i = 0u8;
        while i < n {
            let v = tape_byte(h)?;
            sum = sum.wrapping_add(v);
            h.poke(a, v);
            a = a.wrapping_add(1);
            i += 1;
        }
        if sum.wrapping_add(tape_byte(h)?) != 0 {
            return Err(Cancel);
        }
        if n == 0 {
            return Ok(());
        }
    }
}
