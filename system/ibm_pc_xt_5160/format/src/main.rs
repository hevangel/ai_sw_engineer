//! FORMAT entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `DosCom` makes the INT 21h calls the original makes, reads the drive parameter
//!   block and the INT 21h vector with far accesses, and does INT 25h/26h, INT 11h and INT 13h
//!   through `src/bios.s`. The system files are read into the memory between the end of
//!   `.bss` and the stack.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{entry_ax, far_read16, far_write16, int21, read_byte, Regs};
    use software::{on_break, run, Dos, Dpb, Mem, Saved, FATBUF};

    /// The FAT/directory buffer and the boot record (spec D2, D1).
    static mut MEM: Mem = Mem {
        fat: [0; FATBUF],
        boot: [0; 512],
    };

    /// What the Ctrl-Break handler restores.
    static mut SAVED: Saved = Saved {
        dir: [0; 66],
        restore: false,
    };

    /// Bytes kept for the stack below the top of the program's memory.
    const STACK: u16 = 0x0800;

    extern "C" {
        /// The end of `.bss` (`link_com.ld`): the system-file buffer starts here.
        static __bss_end: u8;
        /// INT 25h / INT 26h; 0, or 100h + the error code (`bios.s`).
        fn format_disk_io(io: *const Io) -> u16;
        /// INT 11h (`bios.s`).
        fn format_equipment() -> u16;
        /// INT 13h (`bios.s`).
        fn format_bios_disk(b: *mut Bios);
    }

    /// The parameter block of `format_disk_io`.
    #[repr(C)]
    struct Io {
        write: u16,
        drive: u16,
        buf: u16,
        count: u16,
        sector: u16,
    }

    /// The parameter block of `format_bios_disk`.
    #[repr(C)]
    struct Bios {
        ax: u16,
        cx: u16,
        dx: u16,
        table: u16,
        carry: u16,
    }

    fn sysbuf() -> u16 {
        (&raw const __bss_end) as usize as u16
    }

    struct DosCom;

    /// INT 21h with AX, CX and DX (the program's segment in DS and ES); the registers DOS
    /// returns. One out-of-line call keeps each call site small.
    #[inline(never)]
    fn dos(ax: u16, cx: u16, dx: u16) -> Regs {
        let mut r = Regs::tiny(ax);
        r.cx = cx;
        r.dx = dx;
        unsafe { int21(&mut r) };
        r
    }

    /// INT 21h with AX and a pointer in DX; AL.
    #[inline(never)]
    fn dos_ptr(ax: u16, p: *const u8) -> u8 {
        dos(ax, 0, p as u16).ax as u8
    }

    /// INT 23h: entered by DOS on Ctrl-Break; never returns (INT_23 at 93Ah: FEXIT).
    extern "C" fn break_handler() {
        #[allow(static_mut_refs)]
        let saved = unsafe { SAVED };
        on_break(&mut DosCom, &saved);
        ibm_pc_dos::exit();
    }

    impl Dos for DosCom {
        fn version(&mut self) -> u16 {
            dos(0x3000, 0, 0).ax
        }
        fn entry_ax(&self) -> u16 {
            entry_ax()
        }
        fn psp(&self, offset: u16) -> u8 {
            read_byte(offset)
        }
        fn peek16(&mut self, segment: u16, offset: u16) -> u16 {
            unsafe { far_read16(segment, offset) }
        }
        fn poke8(&mut self, segment: u16, offset: u16, value: u8) {
            unsafe {
                let w = far_read16(segment, offset);
                far_write16(segment, offset, (w & 0xFF00) | u16::from(value));
            }
        }
        fn current_drive(&mut self) -> u8 {
            dos(0x1900, 0, 0).ax as u8
        }
        fn set_break_handler(&mut self, saved: &Saved) {
            unsafe { SAVED = *saved };
            dos(0x2523, 0, break_handler as usize as u16);
        }
        fn switch_char(&mut self) -> u8 {
            dos(0x3700, 0, 0).dx as u8
        }
        fn disk_reset(&mut self) {
            dos(0x0D00, 0, 0);
        }
        #[inline(never)]
        fn chdir(&mut self, path: &[u8]) -> bool {
            dos(0x3B00, 0, path.as_ptr() as u16).flags & 1 == 0
        }
        fn dpb(&mut self, drive: u8, dir: &mut [u8]) -> Option<Dpb> {
            let r = dos(0x3200, 0, u16::from(drive));
            if r.ax as u8 == 0xFF {
                return None;
            }
            let (s, o) = (r.ds, r.bx);
            let at = |off: u16| unsafe { far_read16(s, o.wrapping_add(off)) };
            for (i, c) in dir.iter_mut().enumerate() {
                *c = at(0x1E + i as u16) as u8;
            }
            Some(Dpb {
                sector_size: at(2),
                cluster_mask: at(4) as u8,
                max_cluster: at(0x0D),
                cur_cluster: at(0x1C),
            })
        }
        #[inline(never)]
        fn fcb(&mut self, function: u8, fcb: &mut [u8]) -> u8 {
            dos_ptr(u16::from(function) << 8, fcb.as_ptr())
        }
        fn set_dta(&mut self, offset: u16) {
            dos(0x1A00, 0, sysbuf().wrapping_add(offset));
        }
        fn block(&mut self, write: bool, fcb: &mut [u8], count: u16) -> (u8, u16) {
            let r = dos(
                if write { 0x2800 } else { 0x2700 },
                count,
                fcb.as_ptr() as u16,
            );
            (r.ax as u8, r.cx)
        }
        #[inline(never)]
        fn print(&mut self, text: &[u8]) {
            dos_ptr(0x0900, text.as_ptr());
        }
        #[inline(never)]
        fn putc(&mut self, c: u8) {
            dos(0x0200, 0, u16::from(c));
        }
        fn read_line(&mut self, buf: &mut [u8]) {
            dos_ptr(0x0A00, buf.as_ptr());
        }
        fn read_key(&mut self, function: u8) -> u8 {
            dos(0x0C00 | u16::from(function), 0, 0).ax as u8
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
            let r = unsafe { format_disk_io(&io) };
            if r == 0 {
                None
            } else {
                Some(r as u8)
            }
        }
        fn equipment(&mut self) -> u16 {
            unsafe { format_equipment() }
        }
        fn bios_disk(&mut self, ax: u16, cx: u16, dx: u16, table: &[u8]) -> (u16, u16, bool) {
            let mut b = Bios {
                ax,
                cx,
                dx,
                table: if table.is_empty() {
                    0
                } else {
                    table.as_ptr() as u16
                },
                carry: 0,
            };
            unsafe { format_bios_disk(&mut b) };
            (b.ax, b.dx, b.carry != 0)
        }
        fn read_file(&mut self, path: &[u8], offset: u16, buf: &mut [u8]) -> bool {
            let r = dos(0x3D00, 0, path.as_ptr() as u16);
            if r.flags & 1 != 0 {
                return false;
            }
            let h = r.ax;
            let mut seek = Regs::tiny(0x4200);
            seek.bx = h;
            seek.dx = offset;
            unsafe { int21(&mut seek) };
            let mut read = Regs::tiny(0x3F00);
            read.bx = h;
            read.cx = buf.len() as u16;
            read.dx = buf.as_mut_ptr() as u16;
            unsafe { int21(&mut read) };
            let mut close = Regs::tiny(0x3E00);
            close.bx = h;
            unsafe { int21(&mut close) };
            seek.flags & 1 == 0 && read.flags & 1 == 0 && read.ax == buf.len() as u16
        }
        fn sys_memory(&self) -> u16 {
            // the program's memory, at most the segment crt0_com.s gives the stack
            let top = read_byte(2) as u16 | (read_byte(3) as u16) << 8;
            let paras = top.wrapping_sub(ibm_pc_dos::cs()).min(0x0FFF);
            (paras << 4).wrapping_sub(STACK).wrapping_sub(sysbuf())
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
    println!("format: the host build only runs the tests (cargo test)");
}
