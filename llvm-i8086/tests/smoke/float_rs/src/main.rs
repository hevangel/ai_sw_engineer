//! A Rust program for the IBM PC 5150 that runs `checks::run`: IEEE 754 single and
//! double arithmetic (software, through compiler_builtins), conversions, and
//! 128-bit integers. The emulator test runs the same source natively on the host
//! and compares the texts bit for bit.

#![no_std]
#![no_main]

mod checks;

use ibm_pc_hal::{halt, Console};

#[no_mangle]
pub extern "C" fn main() -> ! {
    checks::run(&mut Console);
    halt()
}
