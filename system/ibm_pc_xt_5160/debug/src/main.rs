//! DEBUG entry point.
//!
//! * Target (`target_os = "none"`): a load-high PC DOS `.EXE` named DEBUG.COM (spec D1).
//!   `src/start.s` starts it; `src/debug.s` makes the DOS/BIOS calls, reaches arbitrary segments
//!   (the user program and the PSP), builds the user segment with function 26h, and runs the
//!   debugged program with single-step and breakpoints.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos as _;
    use software::{run, Dos, Stop, UserRegs};

    /// The register block of `debug_general`.
    #[repr(C)]
    struct R {
        ax: u16,
        bx: u16,
        cx: u16,
        dx: u16,
        si: u16,
        di: u16,
        ds: u16,
        es: u16,
        flags: u16,
    }

    extern "C" {
        static dos_entry_ax: u16;
        static debug_dg: u16;
        fn debug_out(ch: u16) -> u16;
        fn debug_line(buf_off: u16) -> u16;
        fn debug_columns() -> u16;
        fn debug_create_psb(seg: u16);
        fn debug_set_terminate(h: u16);
        fn debug_peek(seg: u16, off: u16) -> u16;
        fn debug_poke(seg: u16, off: u16, val: u16);
        fn debug_general(r: *mut R) -> u16;
        fn debug_exit() -> !;
        fn debug_run(regs: *mut UserRegs, bps: *const (u16, u16), nbp: u16) -> u16;
    }

    fn dg() -> u16 {
        unsafe { core::ptr::read_volatile(&raw const debug_dg) }
    }

    /// The command line buffer in DG (an area the original keeps at LBUFSIZ; the rewrite uses a
    /// free area of the PSP segment above the tail, 0xE0, 162 bytes).
    const LINEBUF: u16 = 0xE0;

    fn call(ax: u16, bx: u16, cx: u16, dx: u16, ds: u16, es: u16) -> R {
        let mut r = R {
            ax,
            bx,
            cx,
            dx,
            si: 0,
            di: 0,
            ds,
            es,
            flags: 0,
        };
        unsafe { debug_general(&mut r) };
        r
    }

    struct DosT;

    impl Dos for DosT {
        fn entry_ax(&self) -> u16 {
            unsafe { core::ptr::read_volatile(&raw const dos_entry_ax) }
        }
        fn out(&mut self, ch: u8) {
            unsafe { debug_out(u16::from(ch)) };
        }
        fn line(&mut self, buf: &mut [u8]) -> usize {
            let n = unsafe { debug_line(LINEBUF) } as usize;
            let d = dg();
            let count = n.min(buf.len());
            for (i, b) in buf.iter_mut().enumerate().take(count) {
                *b = unsafe { debug_peek(d, LINEBUF + 2 + i as u16) } as u8;
            }
            count
        }
        fn columns(&mut self) -> u8 {
            unsafe { debug_columns() as u8 }
        }
        fn create_psb(&mut self, seg: u16) -> u16 {
            unsafe { debug_create_psb(seg) };
            self.peekw(seg, 6)
        }
        fn dg(&self) -> u16 {
            dg()
        }
        fn peek(&mut self, seg: u16, off: u16) -> u8 {
            unsafe { debug_peek(seg, off) as u8 }
        }
        fn poke(&mut self, seg: u16, off: u16, val: u8) {
            unsafe { debug_poke(seg, off, u16::from(val)) };
        }
        fn set_terminate(&mut self) {
            unsafe { debug_set_terminate(0) };
        }
        fn open(&mut self, name: &[u8]) -> Result<u16, ()> {
            let off = self.store_name(name);
            let r = call(0x3D00, 0, 0, off, dg(), dg());
            if r.flags & 1 != 0 {
                Err(())
            } else {
                Ok(r.ax)
            }
        }
        fn create(&mut self, name: &[u8]) -> Result<u16, ()> {
            let off = self.store_name(name);
            let r = call(0x3C00, 0, 0, off, dg(), dg());
            if r.flags & 1 != 0 {
                Err(())
            } else {
                Ok(r.ax)
            }
        }
        fn seek_end(&mut self, handle: u16) -> u32 {
            let r = call(0x4202, handle, 0, 0, dg(), dg());
            let size = (u32::from(r.dx) << 16) | u32::from(r.ax);
            call(0x4200, handle, 0, 0, dg(), dg());
            size
        }
        fn read(&mut self, handle: u16, seg: u16, off: u16, len: u16) -> u16 {
            call(0x3F00, handle, len, off, seg, seg).ax
        }
        fn write(&mut self, handle: u16, seg: u16, off: u16, len: u16) -> u16 {
            call(0x4000, handle, len, off, seg, seg).ax
        }
        fn close(&mut self, handle: u16) {
            call(0x3E00, handle, 0, 0, dg(), dg());
        }
        fn seek(&mut self, handle: u16, pos: u32) {
            call(0x4200, handle, (pos >> 16) as u16, pos as u16, dg(), dg());
        }
        fn run_user(&mut self, regs: &mut UserRegs, bps: &[(u16, u16)], _trace: bool) -> Stop {
            let r = unsafe { debug_run(regs, bps.as_ptr(), bps.len() as u16) };
            if r == 0 {
                Stop::Reenter
            } else {
                Stop::Terminated
            }
        }
        fn exit(&mut self) -> ! {
            unsafe { debug_exit() }
        }
    }

    fn pokew(seg: u16, off: u16, v: u16) {
        unsafe {
            debug_poke(seg, off, v & 0xFF);
            debug_poke(seg, off + 1, v >> 8);
        }
    }
    fn peekw(seg: u16, off: u16) -> u16 {
        unsafe { debug_peek(seg, off) | (debug_peek(seg, off + 1) << 8) }
    }
    fn setblock(seg: u16, paras: u16) {
        let mut r = R {
            ax: 0x4A00,
            bx: paras,
            cx: 0,
            dx: 0,
            si: 0,
            di: 0,
            ds: seg,
            es: seg,
            flags: 0,
        };
        unsafe { debug_general(&mut r) };
    }

    impl DosT {
        /// Copy a NUL/CR-terminated name into a scratch area of DG and return its offset.
        fn store_name(&mut self, name: &[u8]) -> u16 {
            let base = 0x200u16;
            let d = dg();
            let mut i = 0u16;
            for &c in name {
                if c == 0 || c == 13 {
                    break;
                }
                unsafe { debug_poke(d, base + i, u16::from(c)) };
                i += 1;
            }
            unsafe { debug_poke(d, base + i, 0) };
            base
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        run(&mut DosT);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("debug: the host build only runs the tests (cargo test)");
}
