//! A Rust program for the IBM PC 5150 that runs `checks::run`, the same source
//! the emulator test runs natively on the host to get the expected text.

#![no_std]
#![no_main]

mod checks;

use ibm_pc_hal::{halt, Console};

#[no_mangle]
pub extern "C" fn main() -> ! {
    checks::run(&mut Console);
    halt()
}
