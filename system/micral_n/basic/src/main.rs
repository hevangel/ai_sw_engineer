//! SCELBAL entry point.
//!
//! * Target (`target_os = "none"`, llvm-i8008): `asm/start.s` is at address
//!   0, where the boot ROM starts a loaded program. It calls `basic_init`
//!   and then jumps to `basic_exec`; an error jumps to `basic_restart`,
//!   which jumps to `basic_exec` again.
//! * Host: SCELBAL on stdin/stdout (`cargo run`): newline is CR, keys are
//!   uppercased, output CR is dropped. An error restarts the executive by
//!   a panic caught here.
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
    pub extern "C" fn basic_init() {
        software::power_on();
    }

    #[no_mangle]
    pub extern "C" fn basic_exec() -> ! {
        software::exec(&mut Board)
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
    use std::panic::{catch_unwind, panic_any, AssertUnwindSafe};

    struct Eof;
    struct Tty;
    impl Hal for Tty {
        fn getc(&mut self) -> u8 {
            let mut b = [0u8];
            if std::io::stdin().read(&mut b).unwrap_or(0) == 0 {
                panic_any(Eof);
            }
            let c = b[0] & 0x7f;
            if c == b'\n' {
                b'\r'
            } else {
                c.to_ascii_uppercase()
            }
        }
        fn putc(&mut self, c: u8) {
            if c != b'\r' {
                let _ = std::io::stdout().write_all(&[c]);
                let _ = std::io::stdout().flush();
            }
        }
        fn halt(&mut self) {
            panic_any(Eof);
        }
    }

    std::panic::set_hook(Box::new(|_| {}));
    software::power_on();
    loop {
        let stop = catch_unwind(AssertUnwindSafe(|| software::exec(&mut Tty)));
        if let Err(p) = stop {
            if p.is::<Eof>() {
                return;
            }
        }
    }
}
