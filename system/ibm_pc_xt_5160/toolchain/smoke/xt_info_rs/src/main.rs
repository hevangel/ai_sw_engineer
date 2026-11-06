//! A Rust program for the IBM PC XT 5160 profile: built by the shared
//! llvm-i8086 toolchain, linked with the 5150 platform runtime (the XT has the
//! same CPU, BIOS interface and boot path), booted from a floppy.
//!
//! It prints what makes the XT profile different and what a host can check
//! without running it:
//! - the RAM size the BIOS stored at 0040:0013 (KiB, set by POST) and the
//!   equipment word at 0040:0010 (diskette drives, initial video mode);
//! - a write/read test of one word at the start of every 16 KiB block from
//!   64 KiB to the reported top of RAM, through far pointers (the tiny model
//!   itself only reaches its own 64 KiB segment);
//! - a 16-bit sum of the 8 KiB BIOS ROM at F000:E000, which the test computes
//!   from the ROM file on the host.

#![no_std]
#![no_main]

use ibm_pc_hal::{far_read16, far_write16, halt, println};

/// Checks one word of RAM at `segment:0`, leaving it as it was.
fn probe(segment: u16) -> bool {
    unsafe {
        let old = far_read16(segment, 0);
        let mut ok = true;
        for pattern in [0x5AA5u16, 0xA55A, segment] {
            far_write16(segment, 0, pattern);
            ok &= far_read16(segment, 0) == pattern;
        }
        far_write16(segment, 0, old);
        ok
    }
}

#[no_mangle]
pub extern "C" fn main() -> ! {
    let (kib, equipment) = unsafe { (far_read16(0x40, 0x13), far_read16(0x40, 0x10)) };
    let drives = if equipment & 1 != 0 { ((equipment >> 6) & 3) + 1 } else { 0 };
    let video = match (equipment >> 4) & 3 {
        1 => "40x25 colour",
        2 => "80x25 colour",
        3 => "80x25 mono",
        _ => "none",
    };
    println!("XT 5160 profile");
    println!("ram = {} KiB", kib);
    println!("drives = {}", drives);
    println!("video = {}", video);

    // Every 16 KiB block from 64 KiB to the top of RAM (segment steps of 400h).
    let (mut tested, mut bad) = (0u16, 0u16);
    let top = kib.saturating_mul(64); // KiB -> paragraphs
    let mut seg = 0x1000u16;
    while seg < top {
        tested += 1;
        if !probe(seg) {
            bad += 1;
        }
        seg = seg.wrapping_add(0x400);
        if seg == 0 {
            break;
        }
    }
    println!("blocks above 64 KiB = {}, bad = {}", tested, bad);

    let mut sum = 0u16;
    let mut off = 0xE000u16;
    loop {
        sum = sum.wrapping_add(unsafe { far_read16(0xF000, off) });
        if off == 0xFFFE {
            break;
        }
        off += 2;
    }
    println!("bios sum = {:#06x}", sum);
    println!("done");
    halt()
}
