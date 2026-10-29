//! Mini smoke: hand-verifiable primitive ops. Each result longword is
//! computed from the INPUT words so nothing folds, stored sequentially
//! starting at OUTPUT, then the DONE sentinel. Python mirror in
//! scripts/mini_expected.py; first divergent slot names the broken op.
//!
//! The windows live at 0x1xxxx so the O0 image (whose .text spans
//! 0x400-0x1484) cannot overlap them.
#![no_std]
#![no_main]
use core::ptr;

const INPUT: *const u8 = 0x1_0000 as *const u8;
const OUTPUT: *mut u32 = 0x1_1000 as *mut u32;
const DONE: *mut u8 = 0x1_2000 as *mut u8;

fn inw(i: usize) -> u32 {
    unsafe {
        ptr::read_volatile(INPUT.add(i * 4) as *const u32)
    }
}

#[no_mangle]
pub extern "C" fn _start() -> i32 {
    unsafe {
        let a = inw(0); // 0x12345678
        let b = inw(1); // 0x9ABCDEF0
        OUTPUT.add(0).write_volatile(a.wrapping_add(b)); // 0: add.l
        OUTPUT.add(1).write_volatile(a ^ b); // 1: eor.l
        OUTPUT.add(2).write_volatile(a & b); // 2: and.l
        OUTPUT.add(3).write_volatile(a | b); // 3: or.l
        OUTPUT.add(4).write_volatile(a.rotate_left(4)); // 4: rol
        OUTPUT.add(5).write_volatile(a >> 5); // 5: lsr.l
        OUTPUT.add(6).write_volatile((a as u16 as u32).wrapping_mul(b as u16 as u32)); // 6: mulu
        OUTPUT.add(7).write_volatile(b.wrapping_sub(a)); // 7: sub.l
        OUTPUT.add(8).write_volatile(a.reverse_bits()); // 8: bit-reverse loop
        OUTPUT.add(9).write_volatile(if a > b { a } else { b }); // 9: cmp/select
        OUTPUT.add(10).write_volatile(a.count_ones() as u32); // 10: popcount helper
        OUTPUT.add(11).write_volatile(a.rotate_right(8)); // 11: ror
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

/// compiler_builtins' division helpers reference `abort` at opt-level=0.
#[no_mangle]
pub extern "C" fn abort() -> ! {
    spin()
}
