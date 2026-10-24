//! Micral N boot ROM entry point.
//!
//! * Target (`target_os = "none"`, llvm-i8008): `asm/io.s` calls `boot_main`
//!   at reset and then executes `RST 0`.
//! * Host: loads a disk image file into a 16 KiB array, for trying the core.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use core::panic::PanicInfo;
    use core::ptr::write_volatile;
    use software::Hal;

    extern "C" {
        fn __leaf_disk_command(v: u8);
        fn __leaf_disk_step(dir: u8);
        fn __leaf_disk_read(sector: u8) -> u8;
        fn __leaf_disk_status() -> u8;
        fn __leaf_disk_to_stack() -> u8;
        fn __leaf_stack_pop() -> u8;
    }

    struct Board;
    impl Hal for Board {
        fn command(&mut self, v: u8) {
            unsafe { __leaf_disk_command(v) }
        }
        fn step(&mut self, dir: u8) {
            unsafe { __leaf_disk_step(dir) }
        }
        fn read(&mut self, sector: u8) -> u8 {
            unsafe { __leaf_disk_read(sector) }
        }
        fn status(&mut self) -> u8 {
            unsafe { __leaf_disk_status() }
        }
        fn to_stack(&mut self) {
            unsafe { __leaf_disk_to_stack() };
        }
        fn stack_pop(&mut self) -> u8 {
            unsafe { __leaf_stack_pop() }
        }
        fn poke(&mut self, a: u16, v: u8) {
            unsafe { write_volatile(a as usize as *mut u8, v) }
        }
    }

    #[no_mangle]
    pub extern "C" fn boot_main() {
        software::boot(&mut Board);
    }

    #[panic_handler]
    fn panic(_: &PanicInfo) -> ! {
        loop {}
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("micral_n boot: host build has no disk; see tests/disk_model.rs");
}
