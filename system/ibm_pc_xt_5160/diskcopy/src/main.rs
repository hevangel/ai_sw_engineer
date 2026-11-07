//! DISKCOPY entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `src/start.s` starts it with the stack below the track buffer (spec D1); `DosCom`
//!   makes the INT 21h calls the original makes and INT 13h/INT 11h through `src/bios.s`.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{entry_ax, int21, read_byte, Regs};
    use software::{run, Pc};

    /// The register block of `diskcopy_int13` (`bios.s`).
    #[repr(C)]
    struct Io {
        ax: u16,
        cx: u16,
        dx: u16,
        es: u16,
        bx: u16,
    }

    extern "C" {
        /// INT 13h; 0, or 100h + AH (`bios.s`, spec D2).
        fn diskcopy_int13(io: *const Io) -> u16;
        /// INT 11h (`bios.s`).
        fn diskcopy_int11() -> u16;
        /// The end of the stack, where the track buffer may start (`start.s`).
        fn diskcopy_free() -> u16;
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

    #[inline(never)]
    fn int13(ax: u16, cx: u16, dx: u16, es: u16, bx: u16) -> Result<(), u8> {
        let io = Io { ax, cx, dx, es, bx };
        match unsafe { diskcopy_int13(&io) } {
            0 => Ok(()),
            r => Err(r as u8),
        }
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
            unsafe { diskcopy_free() }
        }
        fn equipment(&mut self) -> u16 {
            unsafe { diskcopy_int11() }
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
        fn disk(&mut self, ax: u16, cx: u16, dx: u16, es: u16) -> Result<(), u8> {
            int13(ax, cx, dx, es, 0)
        }
        fn format(&mut self, ax: u16, cx: u16, dx: u16, table: &[u8; 36]) -> Result<(), u8> {
            int13(ax, cx, dx, ibm_pc_dos::cs(), table.as_ptr() as u16)
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        run(&mut DosCom);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("diskcopy: the host build only runs the tests (cargo test)");
}
