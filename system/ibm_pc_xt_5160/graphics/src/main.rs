//! GRAPHICS entry point.
//!
//! * Target (`target_os = "none"`, built by llvm-i8086 with the XT profile): a PC DOS `.COM`
//!   program. `src/resident.s` comes first: the resident INT 5 handler and its data at the
//!   original's offsets (100h-334h, spec D1/D2) and the start at 335h, which calls `main`.
//!   `DosCom` reads and writes memory directly (the original sets the INT 5 vector without
//!   DOS) and makes the INT 27h call for the core's transient part.
//! * Host: the core is exercised by `tests/oracle_replay.rs`; this binary only says so.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use ibm_pc_dos::{far_read16, far_write16};
    use software::{install, Dos};

    extern "C" {
        /// INT 27h with DX = `bytes` (`resident.s`).
        fn graphics_keep(bytes: u16) -> !;
    }

    struct DosCom;

    impl Dos for DosCom {
        fn cs(&self) -> u16 {
            ibm_pc_dos::cs()
        }
        fn word(&self, segment: u16, offset: u16) -> u16 {
            unsafe { far_read16(segment, offset) }
        }
        fn set_word(&mut self, segment: u16, offset: u16, value: u16) {
            unsafe { far_write16(segment, offset, value) }
        }
        fn set_byte(&mut self, segment: u16, offset: u16, value: u8) {
            unsafe {
                let w = far_read16(segment, offset);
                far_write16(segment, offset, (w & 0xFF00) | u16::from(value));
            }
        }
        fn keep(&mut self, bytes: u16) {
            unsafe { graphics_keep(bytes) }
        }
    }

    #[no_mangle]
    pub extern "C" fn main() {
        install(&mut DosCom);
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("graphics: the host build only runs the tests (cargo test)");
}
