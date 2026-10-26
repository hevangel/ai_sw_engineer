//! Hardware access for programs running on the IBM PC 5150 (real mode, tiny
//! model, 16-bit pointers) built with llvm-i8086.
//!
//! The primitives are assembly (`../runtime/hal.s`, linked with the program);
//! this crate gives them Rust names and a `core::fmt::Write` console, so
//! `println!` works with the BIOS teletype for output.

#![no_std]

use core::fmt;

extern "C" {
    fn hal_putc(c: u8);
    fn hal_halt() -> !;
    fn hal_outb(port: u16, value: u8);
    fn hal_inb(port: u16) -> u8;
    fn hal_far_read16(segment: u16, offset: u16) -> u16;
    fn hal_far_write16(segment: u16, offset: u16, value: u16);
}

/// Writes one byte to the screen through the BIOS teletype (INT 10h).
pub fn putc(c: u8) {
    unsafe { hal_putc(c) }
}

/// Stops the machine for good (interrupts off, `hlt`).
pub fn halt() -> ! {
    unsafe { hal_halt() }
}

/// Writes a byte to an I/O port.
///
/// # Safety
/// The write must be one the device at `port` expects.
pub unsafe fn outb(port: u16, value: u8) {
    hal_outb(port, value)
}

/// Reads a byte from an I/O port.
///
/// # Safety
/// The read must be one the device at `port` allows (some reads have effects).
pub unsafe fn inb(port: u16) -> u8 {
    hal_inb(port)
}

/// Reads a word from `segment:offset`, anywhere in the 1 MiB address space.
///
/// # Safety
/// The address must be readable memory.
pub unsafe fn far_read16(segment: u16, offset: u16) -> u16 {
    hal_far_read16(segment, offset)
}

/// Writes a word to `segment:offset`, anywhere in the 1 MiB address space.
///
/// # Safety
/// The address must be writable memory the program may change.
pub unsafe fn far_write16(segment: u16, offset: u16, value: u16) {
    hal_far_write16(segment, offset, value)
}

/// The text screen: `core::fmt::Write` over the BIOS teletype, `\n` as CR LF.
pub struct Console;

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                putc(b'\r');
            }
            putc(b);
        }
        Ok(())
    }
}

/// `println!` to the screen.
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use ::core::fmt::Write as _;
        let _ = ::core::writeln!($crate::Console, $($arg)*);
    }};
}

#[cfg(feature = "panic-handler")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    for &b in b"\r\npanic\r\n" {
        putc(b);
    }
    halt()
}
