//! PRINT entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `src/resident.s` comes first: the resident part at the original's offsets
//!   (100h-BF9h, spec D1/D2) and the start at BFAh, which calls `main`. `DosCom` makes the
//!   original's INT 21h calls (and INT 2Fh, INT 27h) for the core's transient part.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{cs, far_read16, int21, read_byte, Regs};
    use software::{run, Dos, Hooks, Reply};

    extern "C" {
        /// INT 2Fh with the registers of `r` (`resident.s`).
        fn print_int2f_call(r: *mut Regs);
        /// INT 27h with DX = `bytes` (`resident.s`).
        fn print_keep(bytes: u16) -> !;
        /// The handlers (`resident.s`).
        fn print_int1c();
        fn print_int28();
        fn print_int2f();
        fn print_int13();
        fn print_int17();
        fn print_int14();
        fn print_int5();
    }

    struct DosCom;

    fn call(r: &mut Regs) {
        unsafe { int21(r) };
    }

    fn ptr(b: &[u8]) -> u16 {
        b.as_ptr() as u16
    }

    impl Dos for DosCom {
        fn version(&mut self) -> u16 {
            let mut r = Regs::tiny(0x3000);
            call(&mut r);
            r.ax
        }
        fn print(&mut self, text: &[u8]) {
            let mut r = Regs::tiny(0x0900);
            r.dx = ptr(text);
            call(&mut r);
        }
        fn input(&mut self, buffer: &mut [u8]) {
            let mut r = Regs::tiny(0x0A00);
            r.dx = ptr(buffer);
            call(&mut r);
        }
        fn switch_char(&mut self) -> u8 {
            let mut r = Regs::tiny(0x3700);
            call(&mut r);
            r.dx as u8
        }
        fn get_vector(&mut self, n: u8) -> (u16, u16) {
            let mut r = Regs::tiny(0x3500 | u16::from(n));
            call(&mut r);
            (r.es, r.bx)
        }
        fn set_vector(&mut self, n: u8, offset: u16) {
            let mut r = Regs::tiny(0x2500 | u16::from(n));
            r.dx = offset;
            call(&mut r);
        }
        fn current_drive(&mut self) -> u8 {
            let mut r = Regs::tiny(0x1900);
            call(&mut r);
            r.ax as u8
        }
        fn parse_name(&mut self, name: &[u8], fcb: &mut [u8]) {
            let mut r = Regs::tiny(0x2901);
            r.si = ptr(name);
            r.di = ptr(fcb);
            call(&mut r);
        }
        fn search(&mut self, next: bool, fcb: &mut [u8], dta: &mut [u8]) -> u8 {
            let mut r = Regs::tiny(0x1A00);
            r.dx = ptr(dta);
            call(&mut r);
            let mut r = Regs::tiny(if next { 0x1200 } else { 0x1100 });
            r.dx = ptr(fcb);
            call(&mut r);
            r.ax as u8
        }
        fn open(&mut self, fcb: &mut [u8]) -> u8 {
            let mut r = Regs::tiny(0x0F00);
            r.dx = ptr(fcb);
            call(&mut r);
            r.ax as u8
        }
        fn in_dos(&mut self) -> (u16, u16) {
            let mut r = Regs::tiny(0x3400);
            call(&mut r);
            (r.es, r.bx)
        }
        fn int2f(&mut self, ax: u16, fcb: Option<&[u8]>) -> Reply {
            let mut r = Regs::tiny(ax);
            r.dx = fcb.map_or(0xFFFF, ptr);
            unsafe { print_int2f_call(&mut r) };
            Reply {
                ax: r.ax,
                bx: r.bx,
                dx: r.dx,
                es: r.es,
            }
        }
        fn keep(&mut self, bytes: u16) {
            unsafe { print_keep(bytes) }
        }
        fn end(&mut self) {
            ibm_pc_dos::exit();
        }
        fn peek(&self, offset: u16) -> u8 {
            read_byte(offset)
        }
        fn poke(&mut self, offset: u16, value: u8) {
            unsafe { core::ptr::write_volatile(usize::from(offset) as *mut u8, value) }
        }
        fn far_peek(&self, segment: u16, offset: u16) -> u8 {
            unsafe { far_read16(segment, offset) as u8 }
        }
        fn segment(&self) -> u16 {
            cs()
        }
        fn hooks(&self) -> Hooks {
            Hooks {
                int1c: print_int1c as unsafe extern "C" fn() as usize as u16,
                int28: print_int28 as unsafe extern "C" fn() as usize as u16,
                int2f: print_int2f as unsafe extern "C" fn() as usize as u16,
                int13: print_int13 as unsafe extern "C" fn() as usize as u16,
                int17: print_int17 as unsafe extern "C" fn() as usize as u16,
                int14: print_int14 as unsafe extern "C" fn() as usize as u16,
                int5: print_int5 as unsafe extern "C" fn() as usize as u16,
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        run(&mut DosCom);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("print: the host build only runs the tests (cargo test)");
}
