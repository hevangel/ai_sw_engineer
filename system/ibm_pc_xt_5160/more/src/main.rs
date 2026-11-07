//! MORE entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `DosCom` makes the INT 21h and INT 10h calls the original makes.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{int21, Regs};
    use software::{run, Dos, BUFFER_LEN};

    extern "C" {
        /// INT 10h AH=0Fh; returns AH (`video.s`, spec D3).
        fn more_video_columns() -> u8;
    }

    /// The input buffer, 4,096 bytes as the original's at 21Ah (spec D1).
    static mut BUF: [u8; BUFFER_LEN] = [0; BUFFER_LEN];

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
        fn video_columns(&mut self) -> u8 {
            unsafe { more_video_columns() }
        }
        fn print(&mut self, msg: &[u8]) {
            let mut r = Regs::tiny(0x0900);
            r.dx = msg.as_ptr() as u16;
            call(&mut r);
        }
        fn dup(&mut self, handle: u16) -> u16 {
            let mut r = Regs::tiny(0x4500);
            r.bx = handle;
            call(&mut r);
            r.ax
        }
        fn close(&mut self, handle: u16) {
            let mut r = Regs::tiny(0x3E00);
            r.bx = handle;
            call(&mut r);
        }
        fn read(&mut self, handle: u16, buf: &mut [u8]) -> u16 {
            let mut r = Regs::tiny(0x3F00);
            r.bx = handle;
            r.cx = buf.len() as u16;
            r.dx = buf.as_mut_ptr() as u16;
            call(&mut r);
            r.ax
        }
        fn putc(&mut self, c: u8) {
            let mut r = Regs::tiny(0x0200);
            r.dx = u16::from(c);
            call(&mut r);
        }
        fn key_wait(&mut self) {
            let mut r = Regs::tiny(0x0C01);
            call(&mut r);
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
    println!("more: the host build only runs the tests (cargo test)");
}
