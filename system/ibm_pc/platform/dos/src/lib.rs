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

impl Regs {
    /// A register block for a call that takes pointers: DS and ES are this
    /// program's segment (the tiny model: DS = ES = SS = CS).
    pub fn tiny(ax: u16) -> Regs {
        let segment = cs();
        Regs {
            ax,
            es: segment,
            ds: segment,
            ..Regs::default()
        }
    }
}

extern "C" {
    fn dos_call(regs: *mut Regs);
    fn dos_exit() -> !;
    fn dos_cs() -> u16;
    fn dos_far_read16(segment: u16, offset: u16) -> u16;
    fn dos_far_write16(segment: u16, offset: u16, value: u16);
    fn dos_flush_guarded(target: u8, original: u8, modified: u8);
    /// AX as DOS passed it at entry (`runtime/crt0_com.s`).
    static dos_entry_ax: u16;
}

/// Function 0Dh (writes the disk buffers) with a critical-error handler that ignores a
/// write-protect error on drive `target` unless `fat_modified`; other errors reach the
/// handler DOS had, after drive `original` is selected again. Restores the vector.
pub fn flush_guarded(target: u8, original: u8, fat_modified: bool) {
    unsafe { dos_flush_guarded(target, original, u8::from(fat_modified)) }
}

/// The program's segment (CS, which is also DS, ES and SS).
pub fn cs() -> u16 {
    unsafe { dos_cs() }
}

/// AX at entry: AL is non-zero if the first command-line parameter named an invalid drive.
pub fn entry_ax() -> u16 {
    unsafe { core::ptr::read_volatile(&raw const dos_entry_ax) }
}

/// A byte of this program's segment (the PSP is at offset 0 of it).
pub fn read_byte(offset: u16) -> u8 {
    unsafe { core::ptr::read_volatile(usize::from(offset) as *const u8) }
}

/// A word of this program's segment.
pub fn read_word(offset: u16) -> u16 {
    unsafe { core::ptr::read_volatile(usize::from(offset) as *const u16) }
}

/// The word at `segment:offset`, anywhere in the 1 MiB address space.
///
/// # Safety
/// The address must be readable memory.
pub unsafe fn far_read16(segment: u16, offset: u16) -> u16 {
    dos_far_read16(segment, offset)
}

/// Writes the word at `segment:offset`.
///
/// # Safety
/// The address must be memory the program may change.
pub unsafe fn far_write16(segment: u16, offset: u16, value: u16) {
    dos_far_write16(segment, offset, value)
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
