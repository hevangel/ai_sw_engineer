//! SORT entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS program
//!   (`.EXE`, tiny model). `DosExe` makes the INT 21h calls the original makes.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{int21, read_byte, Regs};
    use software::{run, Dos};

    /// The sort buffer: the original allocates a 64 KiB segment (function 48h); this tiny-model
    /// program keeps a static buffer in its own segment (spec/spec.md, deliberate difference D1).
    /// 32,767 bytes: the largest object Rust allows with 16-bit pointers (isize::MAX).
    const BUFFER: usize = 0x7FFF;
    static mut BUF: [u8; BUFFER] = [0; BUFFER];

    struct DosExe;

    fn call(r: &mut Regs) {
        unsafe { int21(r) };
    }

    impl Dos for DosExe {
        fn version(&mut self) -> (u8, u8) {
            let mut r = Regs::tiny(0x3000);
            call(&mut r);
            (r.ax as u8, (r.ax >> 8) as u8)
        }
        fn switch_char(&mut self) -> u8 {
            let mut r = Regs::tiny(0x3700);
            call(&mut r);
            r.dx as u8
        }
        fn command_tail(&self, i: usize) -> u8 {
            read_byte(0x81 + i as u16)
        }
        fn read_stdin(&mut self, buf: &mut [u8]) -> usize {
            let mut r = Regs::tiny(0x3F00);
            r.bx = 0;
            r.cx = buf.len() as u16;
            r.dx = buf.as_mut_ptr() as u16;
            call(&mut r);
            if r.flags & 1 != 0 {
                0
            } else {
                usize::from(r.ax)
            }
        }
        fn write(&mut self, handle: u16, data: &[u8]) -> Option<usize> {
            let mut r = Regs::tiny(0x4000);
            r.bx = handle;
            r.cx = data.len() as u16;
            r.dx = data.as_ptr() as u16;
            call(&mut r);
            if r.flags & 1 != 0 {
                None
            } else {
                Some(usize::from(r.ax))
            }
        }
        fn print(&mut self, text: &[u8]) {
            for &c in text {
                let mut r = Regs::tiny(0x0200);
                r.dx = u16::from(c);
                call(&mut r);
            }
        }
        fn exit(&mut self, code: u8) {
            let mut r = Regs::tiny(0x4C00 | u16::from(code));
            call(&mut r);
        }
        fn exit_old(&mut self) {
            ibm_pc_dos::exit();
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        #[allow(static_mut_refs)]
        let buf = unsafe { &mut BUF };
        run(&mut DosExe, buf);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("sort: the host build only runs the tests (cargo test)");
}
