//! Representative no_std smoke program for the llvm-m68k qualification
//! (issue hevangel/ai_engineer#64).
//!
//! All inputs come from the INPUT window in memory (`read_volatile`), so
//! nothing can be constant-folded: the compiler must emit real instruction
//! sequences (32-bit loads/stores, multiplies, xors, rotates, shifts,
//! compares, calls into compiler_builtins). Results go to the OUTPUT window
//! and the completion sentinel to DONE; the Python oracle
//! (`scripts/expected.py`) re-implements the same arithmetic independently,
//! and the r68k executor compares memory byte-for-byte.
//!
//! Memory map: spec/compiler_profile.md — INPUT $1000, OUTPUT $1100,
//! DONE $1200; linked at $400; SSP from the reset vector.

#![no_std]
#![no_main]
use core::ptr;

const INPUT: *const u8 = 0x1000 as *const u8;
const OUTPUT: *mut u8 = 0x1100 as *mut u8;
const DONE: *mut u8 = 0x1200 as *mut u8;

fn input(i: usize) -> u8 {
    unsafe { INPUT.add(i).read_volatile() }
}

fn table_entry(i: u8, a: u32, b: u32) -> u8 {
    // Mixes 32-bit multiply, xor, rotates, shifts, a compare-select and a
    // popcount helper call (compiler_builtins) — real lowering paths.
    let wide = (i as u32).wrapping_mul(0x0101_0101) ^ a.rotate_left((i % 7 + 1) as u32);
    let mixed = wide.wrapping_mul(251).wrapping_add(b & 0x00FF_00FF);
    let shifted = (mixed >> 3) | (mixed << (32 - 3));
    let counted = shifted.count_ones() as u32;
    let picked = if counted > (shifted & 0x3F) {
        counted
    } else {
        shifted & 0x3F
    };
    (picked ^ (mixed >> 24)) as u8
}

#[no_mangle]
pub extern "C" fn _start() -> i32 {
    let a = u32::from_be_bytes([input(0), input(1), input(2), input(3)]);
    let b = u32::from_be_bytes([input(4), input(5), input(6), input(7)]);
    let mut i: u8 = 0;
    loop {
        unsafe { OUTPUT.add(i as usize).write_volatile(table_entry(i, a, b)) };
        if i == 31 {
            break;
        }
        i += 1;
    }
    unsafe {
        // 32-bit arithmetic confirmation cell + completion sentinel.
        OUTPUT.add(32 - 2).write_volatile((a.wrapping_mul(b) >> 24) as u8);
        OUTPUT.add(32 - 1).write_volatile(0xA5);
        DONE.write_volatile(0x5A);
    }
    spin()
}

fn spin() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    spin()
}
