//! BASIC entry point.
//!
//! * Target (`target_os = "none"`): a medium-model PC DOS `.EXE` named BASIC.COM (spec D1),
//!   built by `scripts/build.sh` with llvm-i8086's medium-model route (`spec/abi.md` D10):
//!   `rt/medium/start_far.s` starts it, `rt/medium/far.s` gives the BIOS and DOS calls, and
//!   BASIC's data segment and video memory are reached by inline far loads and stores
//!   (`__far_r8` ... which `ir_medium.py` expands).
//! * Host: the core is exercised by `tests/`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use software::hal::Hal;

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
        fn far_int10(r: *mut Regs);
        fn far_int16(r: *mut Regs);
        fn far_exit(code: u16) -> !;
        fn far_shrink() -> u16;
        fn far_alloc(paragraphs: u16) -> u16;
        fn far_largest() -> u16;
        fn far_copy(dseg: u16, doff: u16, sseg: u16, soff: u16, n: u16);
        fn __far_r8(seg: u16, off: u16) -> u8;
        fn __far_w8(seg: u16, off: u16, v: u8);
        fn __far_w16(seg: u16, off: u16, v: u16);
    }

    fn int10(ax: u16, bx: u16, cx: u16, dx: u16) -> Regs {
        let mut r = Regs {
            ax,
            bx,
            cx,
            dx,
            ..Regs::default()
        };
        unsafe { far_int10(&mut r) };
        r
    }

    struct T {
        ds: u16,
        vseg: u16,
        /// The BIOS mode at start (its low bit: colour burst).
        mode: u8,
    }

    impl Hal for T {
        fn peek(&mut self, off: u16) -> u8 {
            unsafe { __far_r8(self.ds, off) }
        }
        fn poke(&mut self, off: u16, v: u8) {
            unsafe { __far_w8(self.ds, off, v) }
        }
        fn vput(&mut self, i: u16, ch: u8, attr: u8) {
            unsafe { __far_w16(self.vseg, i * 2, ch as u16 | (attr as u16) << 8) }
        }
        fn vget(&mut self, i: u16) -> u8 {
            unsafe { __far_r8(self.vseg, i * 2) }
        }
        fn vattr(&mut self, i: u16) -> u8 {
            unsafe { __far_r8(self.vseg, i * 2 + 1) }
        }
        fn vmove(&mut self, dst: u16, src: u16, n: u16) {
            unsafe { far_copy(self.vseg, dst * 2, self.vseg, src * 2, n * 2) }
        }
        // ASSUMPTION (spec M1-W1): the colour-burst bit of the start mode kept; MDA unchanged.
        fn set_width(&mut self, cols: u8) {
            if self.mode == 7 {
                return;
            }
            let m = if cols == 40 {
                self.mode & 1
            } else {
                self.mode & 1 | 2
            };
            int10(m as u16, 0, 0, 0);
        }
        fn set_cursor(&mut self, row: u8, col: u8) {
            int10(0x0200, 0, 0, (row as u16) << 8 | col as u16);
        }
        fn key(&mut self) -> Option<u16> {
            let mut r = Regs::default();
            unsafe { far_int16(&mut r) };
            Some(r.ax)
        }
    }

    #[no_mangle]
    pub extern "C" fn main() -> ! {
        let mode = int10(0x0F00, 0, 0, 0);
        let m = mode.ax as u8;
        let cols = (mode.ax >> 8) as u8;
        let (ds, paragraphs) = unsafe {
            far_shrink();
            let seg = far_alloc(0x1000);
            if seg != 0 {
                (seg, 0x1000)
            } else {
                let n = far_largest();
                (far_alloc(n), n)
            }
        };
        let mut t = T {
            ds,
            vseg: if m == 7 { 0xB000 } else { 0xB800 },
            mode: m,
        };
        software::run(&mut t, if cols == 40 { 40 } else { 80 }, paragraphs);
        unsafe { far_exit(0) }
    }

    #[panic_handler]
    fn panic(_: &core::panic::PanicInfo) -> ! {
        unsafe { far_exit(255) }
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("basic: the host build only runs the tests (cargo test)");
}
