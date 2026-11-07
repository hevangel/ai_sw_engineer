//! COMP entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `DosCom` makes the INT 21h calls the original makes.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{cs, far_read16, int21, read_byte, read_word, Regs};
    use software::{Comp, Dos};

    extern "C" {
        /// The end of `.bss` (i8086-ld): the program's last byte.
        static __bss_end: u8;
    }

    /// Paragraphs kept for the stack below the top of the segment (`crt0_com.s` puts SP
    /// there); the large buffer ends below them (spec/spec.md, D1).
    const STACK_PARAGRAPHS: u16 = 0x80;

    static mut COMP: Comp = Comp::new();

    struct DosCom;

    fn call(r: &mut Regs) {
        unsafe { int21(r) };
    }

    fn set_dta(segment: u16, offset: u16) {
        let mut r = Regs::tiny(0x1A00);
        r.ds = segment;
        r.dx = offset;
        call(&mut r);
    }

    fn fcb_call(ah: u8, fcb: &mut [u8]) -> Regs {
        let mut r = Regs::tiny(u16::from(ah) << 8);
        r.dx = fcb.as_mut_ptr() as u16;
        call(&mut r);
        r
    }

    impl Dos for DosCom {
        fn entry_ax(&self) -> u16 {
            ibm_pc_dos::entry_ax()
        }
        fn version(&mut self) -> u16 {
            let mut r = Regs::tiny(0x3000);
            call(&mut r);
            r.ax
        }
        fn switch_char(&mut self) -> u8 {
            let mut r = Regs::tiny(0x3700);
            call(&mut r);
            r.dx as u8
        }
        fn memory(&self) -> (u16, u16) {
            // 982h: the paragraph after the program; the top as crt0_com.s sets SP from PSP:2.
            let end = unsafe { &raw const __bss_end } as u16;
            let seg = cs();
            let paragraphs = end.wrapping_add(0x10) >> 4;
            let top = read_word(2).wrapping_sub(seg).min(0x0FFF);
            (
                seg.wrapping_add(paragraphs),
                seg.wrapping_add(top).wrapping_sub(STACK_PARAGRAPHS),
            )
        }
        fn default_drive(&mut self) -> u8 {
            let mut r = Regs::tiny(0x1900);
            call(&mut r);
            r.ax as u8
        }
        fn psp(&self, offset: u8) -> u8 {
            read_byte(u16::from(offset))
        }
        fn print(&mut self, msg: &[u8]) {
            let mut r = Regs::tiny(0x0900);
            r.dx = msg.as_ptr() as u16;
            call(&mut r);
        }
        fn putc(&mut self, c: u8) {
            let mut r = Regs::tiny(0x0200);
            r.dx = u16::from(c);
            call(&mut r);
        }
        fn read_line(&mut self, buf: &mut [u8]) {
            let mut r = Regs::tiny(0x0A00);
            r.dx = buf.as_mut_ptr() as u16;
            call(&mut r);
        }
        fn key_echo(&mut self) -> u8 {
            let mut r = Regs::tiny(0x0C01);
            call(&mut r);
            r.ax as u8
        }
        fn parse(&mut self, text: &[u8], fcb: &mut [u8], mode: u8) -> u8 {
            let mut r = Regs::tiny(0x2900 | u16::from(mode));
            r.si = text.as_ptr() as u16;
            r.di = fcb.as_mut_ptr() as u16;
            call(&mut r);
            r.ax as u8
        }
        fn chdir(&mut self, path: &[u8]) -> bool {
            let mut r = Regs::tiny(0x3B00);
            r.dx = path.as_ptr() as u16;
            call(&mut r);
            r.flags & 1 == 0
        }
        fn getcwd(&mut self, drive: u8, buf: &mut [u8]) {
            let mut r = Regs::tiny(0x4700);
            r.dx = u16::from(drive);
            r.si = buf.as_mut_ptr() as u16;
            call(&mut r);
        }
        fn search_first(&mut self, fcb: &mut [u8], dta: &mut [u8]) -> u8 {
            set_dta(cs(), dta.as_mut_ptr() as u16);
            fcb_call(0x11, fcb).ax as u8
        }
        fn search_next(&mut self, fcb: &mut [u8], dta: &mut [u8]) -> u8 {
            set_dta(cs(), dta.as_mut_ptr() as u16);
            fcb_call(0x12, fcb).ax as u8
        }
        fn open(&mut self, fcb: &mut [u8]) -> u8 {
            fcb_call(0x0F, fcb).ax as u8
        }
        fn read_near(&mut self, fcb: &mut [u8], dest: &mut [u8], count: u16) -> (u8, u16) {
            set_dta(cs(), dest.as_mut_ptr() as u16);
            let mut r = Regs::tiny(0x2700);
            r.dx = fcb.as_mut_ptr() as u16;
            r.cx = count;
            call(&mut r);
            (r.ax as u8, r.cx)
        }
        fn read_far(&mut self, fcb: &mut [u8], segment: u16, count: u16) -> (u8, u16) {
            set_dta(segment, 0);
            let mut r = Regs::tiny(0x2700);
            r.dx = fcb.as_mut_ptr() as u16;
            r.cx = count;
            call(&mut r);
            (r.ax as u8, r.cx)
        }
        fn far_byte(&self, segment: u16, offset: u16) -> u8 {
            unsafe { far_read16(segment, offset) as u8 }
        }
        fn exit(&mut self) {
            ibm_pc_dos::exit();
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        #[allow(static_mut_refs)]
        let comp = unsafe { &mut COMP };
        comp.run(&mut DosCom);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("comp: the host build only runs the tests (cargo test)");
}
