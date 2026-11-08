//! DISKCOMP entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `src/start.s` starts it with the stack below the buffers (spec D1); `DosCom`
//!   makes the INT 21h calls the original makes, and INT 13h, INT 11h and the `REPZ CMPSB`
//!   through `src/bios.s`.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{entry_ax, int21, read_byte, Regs};
    use software::{run, Pc};

    /// The register block of `diskcomp_int13` (`bios.s`).
    #[repr(C)]
    struct Io {
        ax: u16,
        cx: u16,
        dx: u16,
        es: u16,
    }

    extern "C" {
        /// INT 13h with BX = 0; 0, or 100h + AH (`bios.s`, spec D2).
        fn diskcomp_int13(io: *const Io) -> u16;
        /// INT 11h (`bios.s`).
        fn diskcomp_int11() -> u16;
        /// REPZ CMPSB of DS:SI against ES:0000 (`bios.s`).
        fn diskcomp_compare(si: u16, es: u16, cx: u16) -> u16;
        /// The end of the stack, where the buffers may start (`start.s`).
        fn diskcomp_free() -> u16;
    }

    struct DosCom;

    /// INT 21h with AX and DX (the program's segment in DS and ES); AX as DOS returns it.
    #[inline(never)]
    fn dos(ax: u16, dx: u16) -> u16 {
        let mut r = Regs::tiny(ax);
        r.dx = dx;
        unsafe { int21(&mut r) };
        r.ax
    }

    impl Pc for DosCom {
        fn dos_major(&mut self) -> u8 {
            dos(0x3000, 0) as u8
        }
        fn entry_ax(&self) -> u16 {
            entry_ax()
        }
        fn psp(&self, offset: u16) -> u8 {
            read_byte(offset)
        }
        fn psp_segment(&self) -> u16 {
            ibm_pc_dos::cs()
        }
        fn free_from(&self) -> u16 {
            unsafe { diskcomp_free() }
        }
        fn equipment(&mut self) -> u16 {
            unsafe { diskcomp_int11() }
        }
        fn current_drive(&mut self) -> u8 {
            dos(0x1900, 0) as u8
        }
        #[inline(never)]
        fn print(&mut self, text: &[u8]) {
            dos(0x0900, text.as_ptr() as u16);
        }
        fn read_key(&mut self) -> u8 {
            dos(0x0C01, 0) as u8
        }
        #[inline(never)]
        fn disk(&mut self, ax: u16, cx: u16, dx: u16, es: u16) -> Result<(), u8> {
            let io = Io { ax, cx, dx, es };
            match unsafe { diskcomp_int13(&io) } {
                0 => Ok(()),
                r => Err(r as u8),
            }
        }
        fn compare(&mut self, si: u16, es: u16, cx: u16) -> u16 {
            unsafe { diskcomp_compare(si, es, cx) }
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        run(&mut DosCom);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("diskcomp: the host build only runs the tests (cargo test)");
}
