//! SCELBAL's memory, page for page (spec "Memory").
//!
//! The original keeps all of its state at fixed addresses in five areas:
//! page 01 (floating point registers, constants, command words), page 26
//! (line and symbol buffers, parser stacks and pointers), page 27 (keyword
//! table, GOSUB and FOR/NEXT stacks, variables), the user program pages
//! 33..54 and page 57 (array values). Pointers into them are kept as page
//! and offset bytes, and much of the program's behaviour (stack overflows,
//! scans past the end of a line) follows from the layout, so this module
//! keeps the same layout and addresses: an address is the original's 14-bit
//! `H:L` value. Addresses outside the five areas (the original's code pages)
//! read as zero and ignore writes (ASSUMPTION (spec BAS-3)).

use core::cell::UnsafeCell;

pub const PG1: u8 = 0o001;
pub const PG26: u8 = 0o026;
pub const PG27: u8 = 0o027;
pub const PG57: u8 = 0o057;
/// First page of the user program buffer (the listing's BGNPGRAM).
pub const BGNPGRAM: u8 = 0o033;
/// First page past it (ENDPGRAM). ASSUMPTION (spec BAS-4): 9 pages, not the
/// listing's 18 (055), so that the image fits the Micral N.
pub const ENDPGRAM: u8 = BGNPGRAM + 9;
const PROG_PAGES: usize = (ENDPGRAM - BGNPGRAM) as usize;
const SIZE: usize = (3 + PROG_PAGES) * 256;

// Page aligned, as the original's areas are: an offset is the low address byte.
#[repr(align(256))]
struct Ram(UnsafeCell<[u8; SIZE]>);
// The machine has one thread; every access goes through the functions below.
unsafe impl Sync for Ram {}
// asm/fp.s addresses page 01 as `basic_ram`.
#[export_name = "basic_ram"]
static RAM: Ram = Ram(UnsafeCell::new(IMAGE));

#[inline(always)]
fn get(i: usize) -> u8 {
    unsafe { *(RAM.0.get() as *const u8).add(i) }
}
#[inline(always)]
fn set(i: usize, v: u8) {
    unsafe { *(RAM.0.get() as *mut u8).add(i) = v }
}

// Fixed-page access, as the listing's `LHI page; LLI offset` pairs.
#[inline(always)]
pub fn g1(o: u8) -> u8 {
    get(o as usize)
}
#[inline(always)]
pub fn s1(o: u8, v: u8) {
    set(o as usize, v)
}
#[inline(always)]
pub fn g26(o: u8) -> u8 {
    get(256 + o as usize)
}
#[inline(always)]
pub fn s26(o: u8, v: u8) {
    set(256 + o as usize, v)
}
#[inline(always)]
pub fn g27(o: u8) -> u8 {
    get(512 + o as usize)
}
#[inline(always)]
pub fn s27(o: u8, v: u8) {
    set(512 + o as usize, v)
}

/// `H:L` for page and offset.
#[inline(always)]
pub const fn at(page: u8, o: u8) -> u16 {
    (page as u16) << 8 | o as u16
}

/// The ARRAY VALUES page (57). ASSUMPTION (spec BAS-2): SCR clears it first,
/// so it needs no place in the image: on the Micral N it is the page the
/// boot ROM uses while loading, free once the program runs (spec "Memory").
#[cfg(target_os = "none")]
fn arrays() -> *mut u8 {
    0x3700 as *mut u8
}

#[cfg(not(target_os = "none"))]
struct Arrays(UnsafeCell<[u8; 256]>);
#[cfg(not(target_os = "none"))]
unsafe impl Sync for Arrays {}
#[cfg(not(target_os = "none"))]
static ARRAYS: Arrays = Arrays(UnsafeCell::new([0; 256]));
#[cfg(not(target_os = "none"))]
fn arrays() -> *mut u8 {
    ARRAYS.0.get() as *mut u8
}

/// Where address `a` lives, if it is SCELBAL data.
#[inline(always)]
fn place(a: u16) -> Option<*mut u8> {
    let page = (a >> 8) as u8 & 0x3F;
    let s = match page {
        PG1 => 0,
        PG26 => 1,
        PG27 => 2,
        PG57 => return Some(unsafe { arrays().add((a as u8) as usize) }),
        p => {
            let q = p.wrapping_sub(BGNPGRAM) as usize;
            if q >= PROG_PAGES {
                return None;
            }
            3 + q
        }
    };
    Some(unsafe { (RAM.0.get() as *mut u8).add(s << 8 | (a as u8) as usize) })
}

/// Read through a pointer (`LAM`).
#[inline(never)]
pub fn rd(a: u16) -> u8 {
    match place(a) {
        Some(p) => unsafe { *p },
        None => 0,
    }
}

/// Write through a pointer (`LMA`).
#[inline(never)]
pub fn wr(a: u16, v: u8) {
    if let Some(p) = place(a) {
        unsafe { *p = v }
    }
}

/// `INL`: the next byte on the same page.
#[inline(always)]
pub fn inl(a: u16) -> u16 {
    a & 0xFF00 | (a as u8).wrapping_add(1) as u16
}

/// `ADV`, `INDEXB`, `ADBDE`: a 16-bit pointer plus a byte.
#[inline(always)]
pub fn add16(a: u16, b: u8) -> u16 {
    a.wrapping_add(b as u16)
}

/// `SUBHL`: a 16-bit pointer minus a byte.
#[inline(always)]
pub fn sub16(a: u16, b: u8) -> u16 {
    a.wrapping_sub(b as u16)
}

/// `MOVEIT`: copy `n` bytes (0 = 256), both pointers advancing on their page.
#[inline(never)]
pub fn moveit(mut src: u16, mut dst: u16, mut n: u8) {
    loop {
        wr(dst, rd(src));
        src = inl(src);
        dst = inl(dst);
        n = n.wrapping_sub(1);
        if n == 0 {
            return;
        }
    }
}

/// Index of `page`:`o` in [`RAM`] (data pages only), at compile time.
const fn ix(page: u8, o: usize) -> usize {
    let s = match page {
        PG1 => 0,
        PG26 => 1,
        PG27 => 2,
        _ => panic!("not an initialised page"),
    };
    s * 256 + o
}

/// One run of the listing's initial data. Text gets bit 7 set on its
/// characters, as SCELBAL stores them (mark parity); counts do not.
const fn put(mut m: [u8; SIZE], page: u8, o: usize, bytes: &[u8], text: bool) -> [u8; SIZE] {
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let marked = text && (b >= 0x20 || b == b'\r' || b == b'\n' || b == 0x14);
        m[ix(page, o) + i] = if marked { b | 0x80 } else { b };
        i += 1;
    }
    m
}

/// The power-on state: the listing's data, everything else zero. The
/// target image carries it, as the original's does; the host copies it in
/// for each session ([`init`]).
const IMAGE: [u8; SIZE] = {
    let m = [0; SIZE];
    // +1.0, then -1.0 (FPACC format: LSW, NSW, MSW, exponent)
    let m = put(m, PG1, 0o004, &[0, 0, 0o100, 1], false);
    let m = put(m, PG1, 0o024, &[0, 0, 0o300, 1], false);
    // RND constants A and C
    let m = put(m, PG1, 0o050, &[1, 0o120, 0o162, 2], false);
    let m = put(m, PG1, 0o060, &[3, 0o150, 0o157, 0o014], false);
    // +10.0 and +0.1
    let m = put(
        m,
        PG1,
        0o210,
        &[0, 0, 0o120, 4, 0o147, 0o146, 0o146, 0o375],
        false,
    );
    let m = put(m, PG1, 0o272, b"\x04SAVE\x04LOAD", true);
    let m = put(
        m,
        PG1,
        0o320,
        b"\x04THEN\x02TO\x04STEP\x04LIST\x03RUN\x03SCR\x0b\x14\r\nREADY\r\n\n\x09 AT LINE ",
        true,
    );
    // Operator hierarchy, entering (240..256) and on the stack (257..275)
    let m = put(
        m,
        PG26,
        0o240,
        &[
            0, 3, 3, 4, 4, 5, 6, 1, 2, 2, 2, 2, 2, 2, 2, 0, 3, 3, 4, 4, 5, 1, 1, 2, 2, 2, 2, 2, 2,
            2,
        ],
        false,
    );
    let m = put(
        m,
        PG26,
        0o300,
        b"\x03INT\x03SGN\x03ABS\x03SQR\x03TAB\x03RND\x03CHR\x03UDF",
        true,
    );
    // The user program pointer starts at the buffer.
    let m = put(m, PG26, 0o360, &[BGNPGRAM], false);
    put(
        m,
        PG27,
        0o000,
        b"\x03REM\x02IF\x03LET\x04GOTO\x05PRINT\x05INPUT\x03FOR\x04NEXT\x05GOSUB\x06RETURN\x03DIM\x03END",
        true,
    )
};

/// Power on: [`IMAGE`] into RAM. The target is loaded with it, so this is
/// for the host, which runs one session after another.
pub fn init() {
    #[cfg(not(target_os = "none"))]
    unsafe {
        *RAM.0.get() = IMAGE;
        *ARRAYS.0.get() = [0; 256];
    }
}
