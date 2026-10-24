//! Micral N boot ROM, reimplemented in Rust (spec/spec.md).
//!
//! `no_std`; written for a 256-byte ROM: byte counters, one loop per step.
#![no_std]

pub mod hal;

pub use hal::{Hal, SECTOR_BYTES};

/// First sector of the image on track 0 (ASSUMPTION spec BOOT-1).
pub const FIRST_SECTOR: u8 = 16;
/// Bytes loaded from address 0 (ASSUMPTION spec BOOT-3).
pub const LOAD_END: u16 = 0x3700;
const DIR_IN: u8 = 0x20;

/// The sector checksum: add each byte, plus one when the addition overflows
/// as a signed number ([MO5emu] `computer_sector_checksum`).
pub fn checksum_step(sum: u8, b: u8) -> u8 {
    let s = sum.wrapping_add(b);
    let overflow = (!(b ^ sum) & (s ^ b)) & 0x80;
    s.wrapping_add(overflow >> 7)
}

/// The address `hi:lo`. A sector buffer is 128-byte aligned, so it never
/// crosses a page: loops step the low byte only.
fn at(hi: u8, lo: u8) -> u16 {
    (hi as u16) << 8 | lo as u16
}

/// Read one sector to `hi:lo` (lo is 0 or 0x80); true when its checksum
/// matches.
fn read_sector<H: Hal>(h: &mut H, sector: u8, hi: u8, lo: u8) -> bool {
    // Select, transfer 131 bytes to the channel, wait (spec step 4, BOOT-4).
    let want = h.read(sector);
    // Channel (last byte first) -> stack card -> disk order.
    h.to_stack();
    let mut sum = 0u8;
    let mut i = lo;
    let end = lo.wrapping_add(128);
    while i != end {
        let b = h.stack_pop();
        h.poke(at(hi, i), b);
        sum = checksum_step(sum, b);
        i = i.wrapping_add(1);
    }
    sum == want
}

/// Load the image. Returns when the caller should execute `RST 0`.
pub fn boot<H: Hal>(h: &mut H) {
    while h.status() & 0x80 == 0 {
        h.step(0);
    }
    let mut sector = FIRST_SECTOR;
    let (mut hi, mut lo) = (0u8, 0u8);
    while hi != (LOAD_END >> 8) as u8 {
        // ASSUMPTION (spec BOOT-2): reselect and re-read a bad sector.
        while !read_sector(h, sector, hi, lo) {
            h.command(sector ^ 1);
        }
        lo ^= 0x80;
        if lo == 0 {
            hi += 1;
        }
        sector += 1;
        if sector == 32 {
            sector = 0;
            h.step(DIR_IN);
        }
    }
}
