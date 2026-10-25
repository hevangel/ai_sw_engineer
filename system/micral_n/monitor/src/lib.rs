//! MIC 01 monitor for the Micral N, reimplemented in Rust (spec/spec.md).
//!
//! `no_std`, no allocation. All machine access goes through [`Hal`].
//!
//! Written for the 8008's size: counters are `u8`, the number being typed and
//! the character that ended it live in statics (a static costs no more than a
//! frame slot on this machine, and passing a tuple back would go through
//! memory), and the shared steps are separate functions.
//!
//! A command that goes wrong prints `?` and the monitor waits for the next
//! command ([`cancel`]). That never returns: the target jumps back to the
//! command loop from wherever it is (`mon_restart` in `asm/entry.s`, a linker
//! `RESET` point; the 8008's return stack is circular, so the abandoned calls
//! cost nothing), and the host unwinds with a panic its caller catches.
//!
//! The holds' code patching is the machine's (`Hal::patch_hold`; on the
//! Micral N, `asm/entry.s`): the Rust reads and checks the `G` line.
#![no_std]

pub mod hal;

pub use hal::{Hal, REG_A, REG_H, REG_LAST};

const CR: u8 = 0x0D;
const LF: u8 = 0x0A;
const DC1: u8 = 0x11; // reader on
const DC2: u8 = 0x12; // punch on
const DC3: u8 = 0x13; // reader off
const DC4: u8 = 0x14; // punch off

/// The number last read by `read_hex` (high and low byte), its digit
/// count, and the character that ended it.
static mut VAL_HI: u8 = 0;
static mut VAL_LO: u8 = 0;
static mut NDIG: u8 = 0;
static mut TERM: u8 = 0;
/// The range a D, P or N command walks: the address at hand and the last.
static mut CUR: u16 = 0;
static mut LAST: u16 = 0;
/// The running byte sum of a tape block: `hex8` adds what it punches,
/// `tape_byte` what it reads.
static mut SUM: u8 = 0;

// Single-threaded machine; the accessors keep the `unsafe` in one place.
fn val() -> u16 {
    unsafe { (VAL_HI as u16) << 8 | VAL_LO as u16 }
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
        VAL_HI = 0;
        VAL_LO = 0;
        NDIG = 0;
        TERM = 0;
    }
}

/// The message a host restart panics with (`cancel`).
pub const RESTART: &str = "MIC 01 restart";

#[cfg(target_os = "none")]
#[inline(always)]
fn restart() -> ! {
    extern "C" {
        /// asm/entry.s: back to the command loop, holds kept.
        fn mon_restart() -> !;
    }
    unsafe { mon_restart() }
}

#[cfg(not(target_os = "none"))]
#[inline(always)]
fn restart() -> ! {
    panic!("{}", RESTART)
}

/// Stop the command: `?`, a new line, and back to the command loop
/// (spec "Commands").
#[inline(never)]
fn cancel<H: Hal>(h: &mut H) -> ! {
    h.putc(b'?');
    crlf(h);
    restart()
}

#[inline(never)]
fn hex_value(c: u8) -> u8 {
    let d = c.wrapping_sub(b'0');
    if d < 10 {
        return d;
    }
    let d = c.wrapping_sub(b'A');
    if d < 6 {
        d + 10
    } else {
        0xFF
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
    unsafe { SUM = SUM.wrapping_add(v) }
    nibble(h, v >> 4);
    nibble(h, v);
}

#[inline(never)]
fn hex16<H: Hal>(h: &mut H, v: u16) {
    hex8(h, (v >> 8) as u8);
    hex8(h, v as u8);
}

/// Hex digits, echoed, into VAL until another character arrives (left in
/// TERM, not echoed). An address may have 4 digits (`?` on a fifth); a
/// byte keeps the last two (§II.3.3).
#[inline(never)]
fn read_hex<H: Hal>(h: &mut H, byte: bool) {
    let mut hi = 0u8;
    let mut lo = 0u8;
    let mut n = 0u8;
    loop {
        let c = h.getc();
        let d = hex_value(c);
        if d == 0xFF {
            unsafe {
                VAL_HI = hi;
                VAL_LO = lo;
                NDIG = n;
                TERM = c;
            }
            return;
        }
        h.putc(c);
        n += 1;
        if !byte && n > 4 {
            cancel(h);
        }
        hi = hi << 4 | lo >> 4;
        lo = lo << 4 | d;
    }
}

/// An address ended by a separator (echoed).
#[inline(never)]
fn read_addr_sep<H: Hal>(h: &mut H) {
    read_hex(h, false);
    let t = term();
    if !is_sep(t) {
        cancel(h);
    }
    h.putc(t);
}

/// An address ended by CR.
#[inline(never)]
fn read_addr_cr<H: Hal>(h: &mut H) {
    read_hex(h, false);
    if term() != CR {
        cancel(h);
    }
}

/// `first , last CR` into CUR and LAST.
#[inline(never)]
fn read_range<H: Hal>(h: &mut H) {
    read_addr_sep(h);
    unsafe { CUR = val() }
    read_addr_cr(h);
    unsafe { LAST = val() }
}

/// The byte at CUR.
#[inline(never)]
fn cur_byte<H: Hal>(h: &mut H) -> u8 {
    h.peek(unsafe { CUR })
}

/// On to the next address; true if CUR was the last of the range (the
/// callers stop there and do not use CUR again).
#[inline(never)]
fn step() -> bool {
    unsafe {
        let c = CUR;
        CUR = c.wrapping_add(1);
        c == LAST
    }
}

/// The program reached hold `n`: restore the holds and say where
/// (ASSUMPTION spec MON-5: CR LF, `*`, the address, CR LF).
pub fn hold_reached<H: Hal>(h: &mut H, n: u8) {
    let a = h.hold_addr(n & 1);
    h.clear_holds();
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
    if !matches!(
        c,
        b'M' | b'A' | b'H' | b'I' | b'T' | b'D' | b'G' | b'C' | b'P' | b'F' | b'N' | b'R'
    ) {
        cancel(h);
    }
    // ASSUMPTION (spec MON-1): the monitor prints the colon.
    h.putc(b':');
    match c {
        b'M' => {
            // The address ends with CR, space or comma (§II.3.3).
            read_hex(h, false);
            let t = term();
            if t == CR {
                crlf(h);
            } else if is_sep(t) {
                h.putc(t);
            } else {
                cancel(h);
            }
            modify(h, val(), true)
        }
        b'A' => modify(h, REG_A as u16, false),
        b'H' => modify(h, REG_H as u16, false),
        // ASSUMPTION (spec MON-3): I and T are the same command; it starts
        // with the flags.
        b'I' | b'T' => modify(h, 0, false),
        b'D' => {
            read_range(h);
            dump(h)
        }
        b'G' => go(h),
        b'C' | b'F' => {
            if h.getc() != CR {
                cancel(h);
            }
            h.putc(DC2);
            if c == b'F' {
                // ASSUMPTION (spec MON-6): the end block before the trailer.
                h.putc(b':');
                repeat(h, b'0', 8);
                crlf(h);
            }
            // ASSUMPTION (spec MON-7): 64 NULs of leader / trailer.
            repeat(h, 0, 64);
            h.putc(DC4);
        }
        b'P' | b'N' => {
            read_range(h);
            crlf(h);
            h.putc(DC2);
            if c == b'P' {
                tape_blocks(h);
            } else {
                bnpf(h);
            }
            h.putc(DC4);
        }
        _ => read_tape(h), // R
    }
}

/// `c`, `n` times.
fn repeat<H: Hal>(h: &mut H, c: u8, n: u8) {
    let mut i = 0u8;
    while i < n {
        h.putc(c);
        i += 1;
    }
}

/// `G:` address [, hold [, hold]] CR (spec "G").
#[inline(never)]
fn go<H: Hal>(h: &mut H) {
    read_hex(h, false);
    let start = val();
    let mut n = 0u8;
    let mut h0 = 0u16;
    let mut h1 = 0u16;
    while is_sep(term()) && n < 2 {
        h.putc(term());
        read_hex(h, false);
        h1 = h0;
        h0 = val();
        n += 1;
    }
    if term() != CR {
        cancel(h);
    }
    // Holds patch 3 bytes each, so they must be at least 3 apart:
    // the two addresses differ by 3..=0xFFFD.
    if n == 2 && h1.wrapping_sub(h0).wrapping_sub(3) > 0xFFFA {
        cancel(h);
    }
    crlf(h);
    h.clear_holds();
    if n != 0 {
        if n == 1 {
            h.patch_hold(0, h0);
        } else {
            h.patch_hold(0, h1);
            h.patch_hold(1, h0);
        }
    }
    h.go(start);
}

/// Display and modify consecutive bytes: memory from `at`, or the saved
/// registers from index `at` (spec "M, A, H, I").
#[inline(never)]
fn modify<H: Hal>(h: &mut H, mut at: u16, memory: bool) {
    loop {
        let r = at as u8;
        if memory {
            let v = h.peek(at);
            // ASSUMPTION (spec MON-2): just the value and the dash.
            hex8(h, v);
        } else if r > REG_LAST {
            return crlf(h);
        } else if r < REG_A {
            // A flag: one digit (ASSUMPTION spec MON-4).
            let v = h.reg(r) & 1;
            h.putc(b'0' + v);
        } else {
            let v = h.reg(r);
            hex8(h, v);
        }
        h.putc(b'-');
        read_hex(h, true);
        let c = term();
        if c == CR {
            return crlf(h);
        }
        if !is_sep(c) {
            cancel(h);
        }
        h.putc(c);
        if ndig() != 0 {
            let b = val() as u8;
            if memory {
                h.poke(at, b);
            } else {
                h.set_reg(r, if r < REG_A { b & 1 } else { b });
            }
        }
        at = at.wrapping_add(1);
    }
}

/// Memory dump of CUR..=LAST, 16 columns per line (spec "D").
#[inline(never)]
fn dump<H: Hal>(h: &mut H) {
    let start = unsafe { CUR };
    unsafe { CUR = start & !0xF }
    loop {
        crlf(h);
        hex16(h, unsafe { CUR });
        loop {
            h.putc(b' ');
            if unsafe { CUR } < start {
                h.putc(b'.');
                h.putc(b'.');
            } else {
                let v = cur_byte(h);
                hex8(h, v);
            }
            if step() {
                return crlf(h);
            }
            if unsafe { CUR } as u8 & 15 == 0 {
                break;
            }
        }
        crlf(h);
    }
}

/// Binary blocks of up to 16 bytes from CUR..=LAST (ASSUMPTION spec MON-6).
#[inline(never)]
fn tape_blocks<H: Hal>(h: &mut H) {
    loop {
        let left = unsafe { LAST.wrapping_sub(CUR) };
        let n: u8 = if left >= 15 { 16 } else { left as u8 + 1 };
        h.putc(b':');
        unsafe { SUM = 0 }
        hex8(h, n);
        hex16(h, unsafe { CUR });
        let mut i = n;
        loop {
            let v = cur_byte(h);
            hex8(h, v);
            i -= 1;
            if step() || i == 0 {
                break;
            }
        }
        hex8(h, unsafe { SUM }.wrapping_neg());
        crlf(h);
        if left < 16 {
            return;
        }
    }
}

/// PROM programming tape of CUR..=LAST, BNPF (ASSUMPTION spec MON-8).
#[inline(never)]
fn bnpf<H: Hal>(h: &mut H) {
    let mut col = 4u8;
    loop {
        let mut v = cur_byte(h);
        h.putc(b'B');
        let mut bit = 8u8;
        while bit != 0 {
            h.putc(if v & 0x80 != 0 { b'P' } else { b'N' });
            v <<= 1;
            bit -= 1;
        }
        h.putc(b'F');
        let last = step();
        col -= 1;
        if col == 0 || last {
            crlf(h);
            col = 4;
        } else {
            h.putc(b' ');
        }
        if last {
            return;
        }
    }
}

/// A bad tape: the reader off, then `?`.
fn tape_error<H: Hal>(h: &mut H) -> ! {
    h.putc(DC3);
    cancel(h)
}

/// One hex character from the tape, not echoed.
#[inline(never)]
fn tape_nibble<H: Hal>(h: &mut H) -> u8 {
    let d = hex_value(h.getc());
    if d == 0xFF {
        tape_error(h);
    }
    d
}

/// Two hex characters from the tape, added to SUM.
#[inline(never)]
fn tape_byte<H: Hal>(h: &mut H) -> u8 {
    let hi = tape_nibble(h);
    let v = hi << 4 | tape_nibble(h);
    unsafe { SUM = SUM.wrapping_add(v) }
    v
}

/// `R`: read blocks until the end block (ASSUMPTION spec MON-9).
#[inline(never)]
fn read_tape<H: Hal>(h: &mut H) {
    h.putc(DC1);
    loop {
        while h.getc() != b':' {}
        unsafe { SUM = 0 }
        let n = tape_byte(h);
        let mut a = (tape_byte(h) as u16) << 8;
        a |= tape_byte(h) as u16;
        let mut i = n;
        while i != 0 {
            let v = tape_byte(h);
            h.poke(a, v);
            a = a.wrapping_add(1);
            i -= 1;
        }
        tape_byte(h);
        if unsafe { SUM } != 0 {
            tape_error(h);
        }
        if n == 0 {
            break;
        }
    }
    h.putc(DC3);
    crlf(h);
}
