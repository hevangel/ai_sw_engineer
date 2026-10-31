//! Representative no_std smoke program for the llvm-mos 6502 qualification
//! (issue hevangel/ai_engineer#58).
//!
//! The program computes a deterministic 32-entry table with the ALU/shift/
//! load-store patterns the backend lowers (adds, subs, multiplies, xors,
//! shifts, rotates, comparisons, 16-bit arithmetic), stores it into a fixed
//! window, then writes the completion sentinel and spins. Expected values
//! are computed INDEPENDENTLY by scripts/expected.py (same arithmetic
//! re-implemented in Python) — never by running the compiled image or the
//! emulator. Two independent executors (vendored py65 and external/mos6502)
//! must produce the window byte-for-byte.
//!
//! No SDK platform is assumed: statics live where the linker script puts
//! them; the runner knows the window/sentinel addresses from the map file.

#![no_std]
#![no_main]

use core::ptr;

// Fixed window (overridden via -C link-arg if needed); addresses are read
// from the map file by the runner, so the exact placement is not assumed.
static mut RESULTS: [u8; 32] = [0; 32];
static mut DONE: u8 = 0;

fn table_entry(i: u8) -> u8 {
    // Deliberately mixes 8-bit add/sub/mul, xor, shifts, rotate, compare and
    // a 16-bit accumulator step so several lowering paths are exercised.
    let wide = (i as u16) * 251 + 0x1234;
    let lo = wide as u8;
    let hi = (wide >> 8) as u8;
    let rolled = lo.rotate_right(3);
    let mixed = (lo ^ hi).wrapping_add(rolled);
    let shifted = ((i << 2) | (i >> 5)).wrapping_sub(hi);
    if mixed > shifted {
        mixed.wrapping_sub(shifted)
    } else {
        shifted.wrapping_sub(mixed)
    }
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    unsafe {
        let base = ptr::addr_of_mut!(RESULTS) as *mut u8;
        let mut i: u8 = 0;
        loop {
            base.add(i as usize).write_volatile(table_entry(i));
            if i == 31 {
                break;
            }
            i += 1;
        }
        // 16-bit arithmetic confirmation cell + completion sentinel; volatile
        // so the computation above is observable and never eliminated.
        base.add(32 - 2)
            .write_volatile(((0x1234u16.wrapping_mul(251)) >> 8) as u8);
        base.add(32 - 1).write_volatile(0xA5);
        ptr::addr_of_mut!(DONE).write_volatile(0x5A);
        spin()
    }
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
