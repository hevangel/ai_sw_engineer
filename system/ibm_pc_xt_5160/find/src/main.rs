//! FIND entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS program
//!   (`.EXE`, tiny model). `DosExe` makes the INT 21h calls the original makes.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{int21, read_byte, Regs};
    use software::{run, Dos, BUFFER_LEN};

    /// The file buffer: 4,096 bytes and the guard byte, as the original's at 3F7h.
    static mut BUF: [u8; BUFFER_LEN] = [0; BUFFER_LEN];

    struct DosExe;

    /// INT 21h; true if DOS returned an error (carry).
    fn call(r: &mut Regs) -> bool {
        unsafe { int21(r) };
        r.flags & 1 != 0
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
        fn open(&mut self, name: &[u8]) -> Option<u16> {
            let mut r = Regs::tiny(0x3D00);
            r.dx = name.as_ptr() as u16;
            if call(&mut r) {
                None
            } else {
                Some(r.ax)
            }
        }
        fn read(&mut self, handle: u16, buf: &mut [u8]) -> Option<usize> {
            let mut r = Regs::tiny(0x3F00);
            r.bx = handle;
            r.cx = buf.len() as u16;
            r.dx = buf.as_mut_ptr() as u16;
            if call(&mut r) {
                None
            } else {
                Some(usize::from(r.ax))
            }
        }
        fn seek_back(&mut self, handle: u16, back: u16) -> bool {
            let mut r = Regs::tiny(0x4201);
            r.bx = handle;
            r.cx = 0xFFFF;
            r.dx = back.wrapping_neg();
            !call(&mut r)
        }
        fn close(&mut self, handle: u16) {
            let mut r = Regs::tiny(0x3E00);
            r.bx = handle;
            call(&mut r);
        }
        fn write(&mut self, handle: u16, data: &[u8]) {
            let mut r = Regs::tiny(0x4000);
            r.bx = handle;
            r.cx = data.len() as u16;
            r.dx = data.as_ptr() as u16;
            call(&mut r);
        }
        fn print(&mut self, text: &[u8]) {
            let mut r = Regs::tiny(0x0900);
            r.dx = text.as_ptr() as u16;
            call(&mut r);
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
    println!("find: the host build only runs the tests (cargo test)");
}
