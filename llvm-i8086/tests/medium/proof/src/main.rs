//! The medium-model proof program (llvm-i8086 spec/abi.md D10): `logic::run` with the far
//! runtime pieces of rt/medium (DOS through `far_int21`, a 64 KiB segment from `far_alloc`)
//! and far data through `__far_r8`/`__far_w8`, which ir_medium.py turns into inline far
//! loads and stores. Built by scripts/build_rust_medium.sh, run by ../tests/dos_run.rs.
#![no_std]
#![no_main]

mod logic;

#[repr(C)]
#[derive(Default)]
struct Regs {
    ax: u16,
    bx: u16,
    cx: u16,
    dx: u16,
    si: u16,
    di: u16,
    es: u16,
    ds: u16,
    flags: u16,
}

extern "C" {
    fn far_int21(r: *mut Regs);
    fn far_exit(code: u16) -> !;
    fn far_shrink() -> u16;
    fn far_alloc(paragraphs: u16) -> u16;
    fn __far_r8(seg: u16, off: u16) -> u8;
    fn __far_w8(seg: u16, off: u16, v: u8);
}

struct T;

impl logic::Hal for T {
    fn putc(&mut self, c: u8) {
        let mut r = Regs {
            ax: 0x0200,
            dx: c as u16,
            ..Regs::default()
        };
        unsafe { far_int21(&mut r) }
    }
    fn far_r8(&mut self, seg: u16, off: u16) -> u8 {
        unsafe { __far_r8(seg, off) }
    }
    fn far_w8(&mut self, seg: u16, off: u16, v: u8) {
        unsafe { __far_w8(seg, off, v) }
    }
}

static SEED: u32 = 0x0BA5_1C00;

#[no_mangle]
pub extern "C" fn main() -> ! {
    let seed = unsafe { core::ptr::read_volatile(&raw const SEED) };
    let seg = unsafe {
        far_shrink();
        far_alloc(0x1000)
    };
    logic::run(&mut T, seed, seg);
    unsafe { far_exit(0) }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe { far_exit(1) }
}
