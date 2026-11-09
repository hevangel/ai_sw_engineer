//! EDLIN entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a load-high PC DOS
//!   `.EXE` named EDLIN.COM (spec D1). `src/start.s` starts it in its own memory at the top;
//!   `DosT` makes the original's INT 21h calls with DS = ES = the program segment prefix's
//!   segment, where the core keeps the original's data and text, through `src/edlin.s`, which
//!   also catches Ctrl-C (INT 23h), chains INT 24h and has the string instructions on that
//!   segment (spec D2).
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use software::{critical_abort, run, Broken, Dos, Regs};

    /// The register block of `edlin_dos` (`edlin.s`).
    #[repr(C)]
    struct R {
        ax: u16,
        bx: u16,
        cx: u16,
        dx: u16,
        si: u16,
        di: u16,
        ds: u16,
        flags: u16,
    }

    /// The block of `edlin_movsb`, `edlin_scasb` and `edlin_cmpsb`.
    #[repr(C)]
    struct S {
        a: u16,
        b: u16,
        c: u16,
        d: u16,
    }

    extern "C" {
        /// The program segment prefix's segment (`start.s`).
        static edlin_seg: u16;
        fn edlin_dos(r: *mut R) -> u16;
        fn edlin_set23();
        fn edlin_hook24();
        fn edlin_unhook24();
        fn edlin_exit() -> !;
        fn edlin_peek(offset: u16) -> u16;
        fn edlin_poke(offset: u16, value: u16);
        fn edlin_far_byte(segment: u16, offset: u16) -> u16;
        fn edlin_movsb(s: *mut S);
        fn edlin_scasb(s: *mut S) -> u16;
        fn edlin_cmpsb(s: *mut S) -> u16;
    }

    fn dg() -> u16 {
        unsafe { core::ptr::read_volatile(&raw const edlin_seg) }
    }

    /// INT 21h through `edlin_dos`; `Err` if Ctrl-C ended it.
    #[inline(never)]
    fn call(r: &mut R) -> Result<(), Broken> {
        if unsafe { edlin_dos(r) } == 0 {
            Ok(())
        } else {
            Err(Broken)
        }
    }

    struct DosT;

    impl Dos for DosT {
        fn entry_ax(&self) -> u16 {
            ibm_pc_dos::entry_ax()
        }
        #[inline(never)]
        fn int21(&mut self, r: &mut Regs) -> Result<(), Broken> {
            let mut b = R {
                ax: r.ax,
                bx: r.bx,
                cx: r.cx,
                dx: r.dx,
                si: r.si,
                di: r.di,
                ds: dg(),
                flags: 0,
            };
            call(&mut b)?;
            *r = Regs {
                ax: b.ax,
                bx: b.bx,
                cx: b.cx,
                dx: b.dx,
                si: b.si,
                di: b.di,
                ds: b.ds,
                carry: b.flags & 1 != 0,
            };
            Ok(())
        }
        #[inline(never)]
        fn print(&mut self, text: &[u8]) -> Result<(), Broken> {
            let mut b = R {
                ax: 0x0900,
                bx: 0,
                cx: 0,
                dx: text.as_ptr() as u16,
                si: 0,
                di: 0,
                ds: ibm_pc_dos::cs(),
                flags: 0,
            };
            call(&mut b)
        }
        fn set_ctrl_c(&mut self) {
            unsafe { edlin_set23() }
        }
        fn hook_critical(&mut self) {
            unsafe { edlin_hook24() }
        }
        fn unhook_critical(&mut self) {
            unsafe { edlin_unhook24() }
        }
        fn exit(&mut self) {
            unsafe { edlin_exit() }
        }
        fn far_byte(&mut self, segment: u16, offset: u16) -> u8 {
            unsafe { edlin_far_byte(segment, offset) as u8 }
        }
        fn peek(&mut self, offset: u16) -> u8 {
            unsafe { edlin_peek(offset) as u8 }
        }
        fn poke(&mut self, offset: u16, value: u8) {
            unsafe { edlin_poke(offset, u16::from(value)) }
        }
        fn movsb(&mut self, di: u16, si: u16, cx: u16, down: bool) {
            let mut s = S {
                a: di,
                b: si,
                c: cx,
                d: u16::from(down),
            };
            unsafe { edlin_movsb(&mut s) }
        }
        fn scasb(&mut self, di: u16, cx: u16, al: u8, down: bool) -> (u16, u16, bool) {
            let mut s = S {
                a: di,
                b: cx,
                c: u16::from(al),
                d: u16::from(down),
            };
            let z = unsafe { edlin_scasb(&mut s) } != 0;
            (s.a, s.b, z)
        }
        fn cmpsb(&mut self, si: u16, di: u16, cx: u16) -> (u16, u16, u16, bool) {
            let mut s = S {
                a: si,
                b: di,
                c: cx,
                d: 0,
            };
            let z = unsafe { edlin_cmpsb(&mut s) } != 0;
            (s.a, s.b, s.c, z)
        }
    }

    /// INT_24_BACK's Abort (`edlin.s`, on the program's stack): REST_DIR, INT 20h.
    #[no_mangle]
    pub extern "C" fn edlin_critical_abort() {
        critical_abort(&mut DosT);
        unsafe { edlin_exit() }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        run(&mut DosT);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("edlin: the host build only runs the tests (cargo test)");
}
