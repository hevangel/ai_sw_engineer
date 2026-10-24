//! Shooting Stars entry point.
//!
//! * Target (`target_os = "none"`, llvm-i8008): `asm/start.s` is at address
//!   0, where the boot ROM starts a loaded program; it calls `game_main`.
//! * Host: play on stdin/stdout (`cargo run`).
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use core::panic::PanicInfo;
    use software::Hal;

    extern "C" {
        /// platform/runtime/tty.s
        fn __leaf_tty_getc() -> u8;
        fn __leaf_tty_putc(c: u8);
        /// asm/start.s
        fn __leaf_halt();
    }

    struct Board;
    impl Hal for Board {
        fn getc(&mut self) -> u8 {
            unsafe { __leaf_tty_getc() }
        }
        fn putc(&mut self, c: u8) {
            unsafe { __leaf_tty_putc(c) }
        }
        fn halt(&mut self) {
            unsafe { __leaf_halt() }
        }
    }

    #[no_mangle]
    pub extern "C" fn game_main() {
        software::play(&mut Board);
    }

    #[panic_handler]
    fn panic(_: &PanicInfo) -> ! {
        loop {}
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    use software::Hal;
    use std::io::{Read, Write};

    struct Tty;
    impl Hal for Tty {
        fn getc(&mut self) -> u8 {
            let mut b = [0u8];
            if std::io::stdin().read(&mut b).unwrap_or(0) == 0 {
                std::process::exit(0);
            }
            let c = b[0] & 0x7f;
            if c == b'\n' {
                b'\r'
            } else {
                c.to_ascii_uppercase()
            }
        }
        fn putc(&mut self, c: u8) {
            let c = if c == b'\r' { b'\n' } else { c };
            let _ = std::io::stdout().write_all(&[c]);
            let _ = std::io::stdout().flush();
        }
        fn halt(&mut self) {
            std::process::exit(0);
        }
    }
    software::play(&mut Tty);
}
