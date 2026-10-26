//! PC DOS 1.x function calls for `.COM` programs (real mode, tiny model),
//! built on one assembly primitive, `dos_call` in `../runtime/dos.s`.
//!
//! The program is linked with `link_com.ld` and `runtime/crt0_com.s`.

#![no_std]

use core::fmt;

/// The registers of a DOS call, in the order `dos_call` loads and stores them.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Regs {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub es: u16,
    pub ds: u16,
    pub flags: u16,
}

extern "C" {
    fn dos_call(regs: *mut Regs);
    fn dos_exit() -> !;
}

/// Executes INT 21h with `regs` and stores the registers it returns.
///
/// # Safety
/// The call must be one DOS accepts with these registers, and any pointer in
/// them must be valid for it.
pub unsafe fn int21(regs: &mut Regs) {
    dos_call(regs)
}

/// Function 02h: writes one character to the console.
pub fn putc(c: u8) {
    let mut r = Regs {
        ax: 0x0200,
        dx: u16::from(c),
        ..Regs::default()
    };
    unsafe { int21(&mut r) }
}

/// INT 20h: ends the program.
pub fn exit() -> ! {
    unsafe { dos_exit() }
}

/// The console: `core::fmt::Write` over function 02h, `\n` as CR LF.
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

/// `println!` to the DOS console.
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
    exit()
}
