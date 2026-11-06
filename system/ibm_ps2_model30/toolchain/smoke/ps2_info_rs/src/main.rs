//! A Rust program for the IBM PS/2 Model 30 profile: built by the shared llvm-i8086 toolchain,
//! linked with the PS/2 platform's crt0 (720 KiB diskette) and the shared HAL, booted from drive A.
//!
//! It prints what makes the Model 30 profile different, each value checkable without running it:
//! - the RAM size the BIOS stored at 0040:0013 (KiB) and the diskette drives of the equipment
//!   word at 0040:0010;
//! - the display combination code of INT 10h AX=1A00h (`ps2.s`): MCGA on the Model 30;
//! - the model byte at F000:FFFE and the BIOS date at F000:FFF5, read from the ROM;
//! - a write/read test of one word at the start of every 16 KiB block from 64 KiB to the top of
//!   RAM, through far pointers;
//! - a 16-bit sum of the 64 KiB BIOS ROM at F000:0000, which the test computes from the ROM file.

#![no_std]
#![no_main]

use ibm_pc_hal::{far_read16, far_write16, halt, println, putc};

extern "C" {
    fn ps2_display_combination() -> u16;
}

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
    let dcc = unsafe { ps2_display_combination() };
    let display = match dcc & 0xFF {
        0x0B => "MCGA mono",
        0x0C => "MCGA colour",
        0x08 => "VGA colour",
        _ => "other",
    };
    let model = unsafe { far_read16(0xF000, 0xFFFE) } & 0xFF;
    println!("PS/2 Model 30 profile");
    println!("ram = {} KiB", kib);
    println!("drives = {}", drives);
    println!("display = {} ({:#04x})", display, dcc & 0xFF);
    println!("model byte = {:#04x}", model);
    let mut date = [0u8; 8];
    for (i, pair) in date.chunks_mut(2).enumerate() {
        let w = unsafe { far_read16(0xF000, 0xFFF5 + 2 * i as u16) };
        pair[0] = w as u8;
        pair[1] = (w >> 8) as u8;
    }
    for &b in b"bios date = " {
        putc(b);
    }
    for b in date {
        putc(b);
    }
    println!();

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
    let mut off = 0u16;
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
