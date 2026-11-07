//! CHKDSK entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `DosCom` makes the INT 21h calls the original makes, reads the drive parameter
//!   block in DOS's memory with far accesses, and does INT 25h/26h through `src/disk.s`.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{entry_ax, far_read16, far_write16, int21, read_byte, Regs};
    use software::{on_break, run, Dos, Saved, MEMORY};

    /// The FAT, the cluster map and the sector buffer (spec D1).
    static mut MEM: [u8; MEMORY] = [0; MEMORY];

    /// What the Ctrl-Break handler restores (spec D4).
    static mut SAVED: Saved = Saved {
        dir: [0; 65],
        drive: 0,
    };

    /// The parameter block of `chkdsk_disk_io` (`disk.s`).
    #[repr(C)]
    struct Io {
        write: u16,
        drive: u16,
        buf: u16,
        count: u16,
        sector: u16,
    }

    extern "C" {
        /// INT 25h / INT 26h; 0, or 100h + the error code (`disk.s`, spec D3).
        fn chkdsk_disk_io(io: *const Io) -> u16;
    }

    /// The DPB function 32h returned: segment, offset.
    static mut DPB: (u16, u16) = (0, 0);

    struct DosCom;

    /// INT 21h with AX and DX (the program's segment in DS and ES); the registers DOS
    /// returns. One out-of-line call keeps each call site small (spec D6).
    #[inline(never)]
    fn dos(ax: u16, dx: u16) -> Regs {
        let mut r = Regs::tiny(ax);
        r.dx = dx;
        unsafe { int21(&mut r) };
        r
    }

    /// INT 21h with AX and a pointer in DX; AL.
    #[inline(never)]
    fn dos_ptr(ax: u16, p: *const u8) -> u8 {
        dos(ax, p as u16).ax as u8
    }

    fn dpb_at(offset: u16) -> (u16, u16) {
        let (s, o) = unsafe { DPB };
        (s, o.wrapping_add(offset))
    }

    /// INT 23h: entered by DOS on Ctrl-Break with this program's segments and stack; never
    /// returns (INT_23 at 113Ch: the user's directory and drive back, INT 20h).
    extern "C" fn break_handler() {
        #[allow(static_mut_refs)]
        let saved = unsafe { SAVED };
        on_break(&mut DosCom, &saved);
        ibm_pc_dos::exit();
    }

    impl Dos for DosCom {
        fn version(&mut self) -> u16 {
            dos(0x3000, 0).ax
        }
        fn entry_ax(&self) -> u16 {
            entry_ax()
        }
        fn psp(&self, offset: u16) -> u8 {
            read_byte(offset)
        }
        fn cs(&self) -> u16 {
            ibm_pc_dos::cs()
        }
        fn disk_reset(&mut self) {
            dos(0x0D00, 0);
        }
        fn switch_char(&mut self) -> u8 {
            dos(0x3700, 0).dx as u8
        }
        fn current_drive(&mut self) -> u8 {
            dos(0x1900, 0).ax as u8
        }
        fn select_drive(&mut self, drive: u8) {
            dos(0x0E00, u16::from(drive));
        }
        fn get_dpb(&mut self, drive: u8) -> bool {
            let r = dos(0x3200, u16::from(drive));
            unsafe { DPB = (r.ds, r.bx) };
            r.ax as u8 != 0xFF
        }
        #[inline(never)]
        fn dpb(&mut self, offset: u16) -> u8 {
            let (s, o) = dpb_at(offset);
            unsafe { far_read16(s, o) as u8 }
        }
        fn set_dpb(&mut self, offset: u16, value: u8) {
            let (s, o) = dpb_at(offset);
            unsafe {
                let w = far_read16(s, o);
                far_write16(s, o, (w & 0xFF00) | u16::from(value));
            }
        }
        fn set_break_handler(&mut self, saved: &Saved) {
            unsafe { SAVED = *saved };
            dos(0x2523, break_handler as usize as u16);
        }
        #[inline(never)]
        fn chdir(&mut self, path: &[u8]) -> bool {
            dos(0x3B00, path.as_ptr() as u16).flags & 1 == 0
        }
        #[inline(never)]
        fn search(&mut self, next: bool, fcb: &mut [u8], dta: &mut [u8]) -> u8 {
            // spec D2: the DTA is the core's record, set before each search.
            dos_ptr(0x1A00, dta.as_ptr());
            dos_ptr(if next { 0x1200 } else { 0x1100 }, fcb.as_ptr())
        }
        fn open(&mut self, fcb: &mut [u8]) -> u8 {
            dos_ptr(0x0F00, fcb.as_ptr())
        }
        #[inline(never)]
        fn disk_io(
            &mut self,
            write: bool,
            drive: u8,
            sector: u16,
            count: u16,
            buf: &mut [u8],
        ) -> Option<u8> {
            let io = Io {
                write: u16::from(write),
                drive: u16::from(drive),
                buf: buf.as_mut_ptr() as u16,
                count,
                sector,
            };
            let r = unsafe { chkdsk_disk_io(&io) };
            if r == 0 {
                None
            } else {
                Some(r as u8)
            }
        }
        #[inline(never)]
        fn print(&mut self, text: &[u8]) {
            dos_ptr(0x0900, text.as_ptr());
        }
        #[inline(never)]
        fn putc(&mut self, c: u8) {
            dos(0x0200, u16::from(c));
        }
        fn read_line(&mut self, buf: &mut [u8]) {
            dos_ptr(0x0A00, buf.as_ptr());
        }
        fn read_key(&mut self) -> u8 {
            dos(0x0C01, 0).ax as u8
        }
        fn exit(&mut self) {
            ibm_pc_dos::exit();
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        #[allow(static_mut_refs)]
        let mem = unsafe { &mut MEM };
        run(&mut DosCom, mem);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("chkdsk: the host build only runs the tests (cargo test)");
}
