//! A Rust program for the IBM PC 5150, built by rustc (as an msp430 program;
//! see llvm-i8086/rust/README.md), retargeted and compiled by our llc, linked
//! by i8086-ld and booted from a floppy. It prints values whose computation
//! needs 16-bit arithmetic, 32- and 64-bit helpers from the runtime library,
//! slices, iterators and `core::fmt`. The test in
//! system/ibm_pc/emulator/tests/i8086_boot.rs computes the same text on the
//! host and compares it with the screen.

#![no_std]
#![no_main]

use ibm_pc_hal::{halt, println};

fn fib(n: u16) -> u16 {
    let (mut a, mut b) = (0u16, 1u16);
    for _ in 0..n {
        (a, b) = (b, a.wrapping_add(b));
    }
    a
}

fn factorial(n: u32) -> u32 {
    (1..=n).product()
}

fn insertion_sort(v: &mut [u16]) {
    for i in 1..v.len() {
        let mut j = i;
        while j > 0 && v[j - 1] > v[j] {
            v.swap(j - 1, j);
            j -= 1;
        }
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[no_mangle]
pub extern "C" fn main() -> ! {
    println!("Rust on the 8088");
    println!("fib(20) = {}", fib(20));
    println!("12! = {}", factorial(12));
    println!("2^40 = {}", 1u64 << 40);
    println!("u64 = {}", 0xFEDC_BA98_7654_3210u64 / 1_000_003);
    println!("i16 = {}", -12345i16 / 7);
    println!("gcd = {}", gcd(1_071_000_000, 462_000_000));
    let mut v = [34u16, 8, 21, 1, 13, 5, 2, 3];
    insertion_sort(&mut v);
    println!("sorted = {:?}", v);
    println!("sum = {}", (1..=100u16).sum::<u16>());
    println!("hex = {:#06x}", 0xBEEFu16);
    println!("done");
    halt()
}
