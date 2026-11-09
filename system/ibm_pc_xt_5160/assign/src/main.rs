//! ASSIGN entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `src/resident.s` comes first: the resident data and hooks at the original's
//!   offsets (100h-302h, spec D1/D2) and the start at 303h, which calls `main`. `DosCom` makes
//!   the original's INT 21h calls (and INT 11h) for the core's transient part.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{far_read16, far_write16, int21, read_byte, Regs};
    use software::{run, Dos};

    extern "C" {
        /// INT 11h (`resident.s`).
        fn assign_int11() -> u16;
        /// The INT 25h and INT 26h hooks (`resident.s`).
        fn assign_int25();
        fn assign_int26();
    }

    struct DosCom;

    fn call(r: &mut Regs) {
        unsafe { int21(r) };
    }

    impl Dos for DosCom {
        fn version(&mut self) -> u16 {
            let mut r = Regs::tiny(0x3000);
            call(&mut r);
            r.ax
        }
        fn print(&mut self, text: &[u8]) -> u8 {
            let mut r = Regs::tiny(0x0900);
            r.dx = text.as_ptr() as u16;
            call(&mut r);
            r.ax as u8
        }
        fn end(&mut self) {
            ibm_pc_dos::exit();
        }
        fn terminate(&mut self, code: u8) {
            let mut r = Regs::tiny(0x4C00 | u16::from(code));
            call(&mut r);
        }
        fn keep(&mut self, code: u8, paragraphs: u16) {
            let mut r = Regs::tiny(0x3100 | u16::from(code));
            r.dx = paragraphs;
            call(&mut r);
        }
        fn equipment(&mut self) -> u16 {
            unsafe { assign_int11() }
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
        fn far_poke(&mut self, segment: u16, offset: u16, value: u8) {
            unsafe {
                let w = far_read16(segment, offset);
                far_write16(segment, offset, (w & 0xFF00) | u16::from(value));
            }
        }
        fn get_vector(&mut self, n: u8) -> (u16, u16) {
            let mut r = Regs::tiny(0x3500 | u16::from(n));
            call(&mut r);
            (r.es, r.bx)
        }
        fn set_vector(&mut self, n: u8, offset: u16) -> u8 {
            let mut r = Regs::tiny(0x2500 | u16::from(n));
            r.dx = offset;
            call(&mut r);
            r.ax as u8
        }
        fn current_drive(&mut self) -> u8 {
            let mut r = Regs::tiny(0x1900);
            call(&mut r);
            r.ax as u8
        }
        fn select_drive(&mut self, drive: u8) {
            let mut r = Regs::tiny(0x0E00);
            r.dx = u16::from(drive);
            call(&mut r);
        }
        fn disk_hooks(&self) -> (u16, u16) {
            (assign_int25 as usize as u16, assign_int26 as usize as u16)
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        run(&mut DosCom);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("assign: the host build only runs the tests (cargo test)");
}
