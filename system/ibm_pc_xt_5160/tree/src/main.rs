//! TREE entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `DosCom` makes the INT 21h calls the original makes.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{entry_ax, int21, read_byte, Regs};
    use software::{restore, run, Dos, Saved, FCB};

    /// The path and the name list (564h onwards in the original, up to PSP:0006h): spec D1.
    const BUFFER: usize = 0x4000;
    static mut BUF: [u8; BUFFER] = [0; BUFFER];

    /// What the Ctrl-Break handler restores (spec D4).
    static mut SAVED: Saved = Saved {
        dir: [0; 65],
        drive: 0,
    };

    struct DosCom;

    fn call(r: &mut Regs) {
        unsafe { int21(r) };
    }

    /// INT 23h: entered by DOS on Ctrl-Break with this program's segments and stack; never
    /// returns (13Ah: restore, INT 20h).
    extern "C" fn on_break() {
        #[allow(static_mut_refs)]
        let saved = unsafe { SAVED };
        restore(&mut DosCom, &saved);
        ibm_pc_dos::exit();
    }

    impl Dos for DosCom {
        fn version(&mut self) -> (u8, u8) {
            let mut r = Regs::tiny(0x3000);
            call(&mut r);
            (r.ax as u8, (r.ax >> 8) as u8)
        }
        fn entry_ax(&self) -> u16 {
            entry_ax()
        }
        fn psp(&self, offset: u16) -> u8 {
            read_byte(offset)
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
        fn switch_char(&mut self) -> u8 {
            let mut r = Regs::tiny(0x3700);
            call(&mut r);
            r.dx as u8
        }
        fn set_break_handler(&mut self, saved: &Saved) {
            unsafe { SAVED = *saved };
            let mut r = Regs::tiny(0x2523);
            r.dx = on_break as usize as u16;
            call(&mut r);
        }
        fn current_dir(&mut self, drive: u8, out: &mut [u8; 64]) {
            let mut r = Regs::tiny(0x4700);
            r.dx = u16::from(drive);
            r.si = out.as_mut_ptr() as u16;
            call(&mut r);
        }
        fn chdir(&mut self, path: &[u8]) -> bool {
            let mut r = Regs::tiny(0x3B00);
            r.dx = path.as_ptr() as u16;
            call(&mut r);
            r.flags & 1 == 0
        }
        fn search(&mut self, next: bool, fcb: &mut [u8; FCB], dta: &mut [u8; FCB]) -> u8 {
            // spec D3: the DTA is the core's record, set before each search.
            let mut r = Regs::tiny(0x1A00);
            r.dx = dta.as_mut_ptr() as u16;
            call(&mut r);
            let mut r = Regs::tiny(if next { 0x1200 } else { 0x1100 });
            r.dx = fcb.as_mut_ptr() as u16;
            call(&mut r);
            r.ax as u8
        }
        fn write(&mut self, handle: u16, data: &[u8]) {
            let mut r = Regs::tiny(0x4000);
            r.bx = handle;
            r.cx = data.len() as u16;
            r.dx = data.as_ptr() as u16;
            call(&mut r);
        }
        fn print(&mut self, text: &[u8]) {
            for &c in text {
                let mut r = Regs::tiny(0x0200);
                r.dx = u16::from(c);
                call(&mut r);
            }
        }
        fn exit(&mut self) {
            ibm_pc_dos::exit();
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        #[allow(static_mut_refs)]
        let buf = unsafe { &mut BUF };
        run(&mut DosCom, buf);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("tree: the host build only runs the tests (cargo test)");
}
