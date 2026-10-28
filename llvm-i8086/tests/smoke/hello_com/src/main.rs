//! A Rust `.COM` program for PC DOS 1.00 on the IBM PC 5150: prints through
//! INT 21h, reads the drive state back through the register block and ends
//! with INT 20h. The emulator and RTL tests in
//! system/ibm_pc/emulator/tests/i8086_dos.rs run it under the real PC DOS 1.00.

#![no_std]
#![no_main]

use ibm_pc_dos::{exit, int21, println, Regs};

#[no_mangle]
pub extern "C" fn main() -> ! {
    println!("Rust under DOS");
    // Function 19h: current drive (0 = A). Function 0Eh: select a drive, returns the drive count.
    let mut r = Regs {
        ax: 0x1900,
        ..Regs::default()
    };
    unsafe { int21(&mut r) };
    let drive = r.ax & 0xFF;
    let mut r = Regs {
        ax: 0x0E00,
        dx: drive,
        ..Regs::default()
    };
    unsafe { int21(&mut r) };
    println!("drive {} of {}", (b'A' + drive as u8) as char, r.ax & 0xFF);
    println!("12! = {}", (1..=12u32).product::<u32>());
    exit()
}
