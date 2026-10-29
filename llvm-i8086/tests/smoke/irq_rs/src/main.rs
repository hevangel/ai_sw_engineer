//! A Rust program for the IBM PC 5150 that takes over the timer (IRQ0, vector
//! 08h) and keyboard (IRQ1, vector 09h) interrupts with handlers written in
//! Rust (`extern "msp430-interrupt"`, which llvm-i8086 compiles as interrupt
//! handlers: spec/abi.md D5). It waits for five timer ticks, prints `ticks ok`,
//! waits for six scan codes from the keyboard, prints them and `done`.
//!
//! The tests in system/ibm_pc/emulator/tests/i8086_irq.rs run it on the Rust
//! emulator and compare with the screen the hardware project's RTL showed.

#![no_std]
#![no_main]
#![feature(abi_msp430_interrupt)]

use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
use ibm_pc_hal::{cli, halt, inb, outb, println, set_vector, sti};

static mut TICKS: u16 = 0;
static mut KEYS: [u8; 8] = [0; 8];
static mut NKEYS: u16 = 0;

/// IRQ0: counts a tick and tells the interrupt controller it is done.
extern "msp430-interrupt" fn timer() {
    unsafe {
        write_volatile(addr_of_mut!(TICKS), read_volatile(addr_of!(TICKS)) + 1);
        outb(0x20, 0x20);
    }
}

/// IRQ1: reads the scan code, acknowledges the keyboard the way the BIOS does
/// (bit 7 of port 61h up and down), stores the code, ends the interrupt.
extern "msp430-interrupt" fn keyboard() {
    unsafe {
        let code = inb(0x60);
        let b = inb(0x61);
        outb(0x61, b | 0x80);
        outb(0x61, b & 0x7F);
        let n = read_volatile(addr_of!(NKEYS));
        if n < 8 {
            write_volatile(addr_of_mut!(KEYS[n as usize]), code);
            write_volatile(addr_of_mut!(NKEYS), n + 1);
        }
        outb(0x20, 0x20);
    }
}

fn ticks() -> u16 {
    unsafe { read_volatile(addr_of!(TICKS)) }
}

fn nkeys() -> u16 {
    unsafe { read_volatile(addr_of!(NKEYS)) }
}

#[no_mangle]
pub extern "C" fn main() -> ! {
    println!("irq test");
    unsafe {
        set_vector(0x08, timer as *const () as usize);
        set_vector(0x09, keyboard as *const () as usize);
    }
    sti();
    while ticks() < 5 {}
    println!("ticks ok");
    while nkeys() < 6 {}
    cli();
    for i in 0..6 {
        println!("key {:02x}", unsafe { read_volatile(addr_of!(KEYS[i])) });
    }
    println!("done");
    halt()
}
