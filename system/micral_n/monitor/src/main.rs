//! MIC 01 monitor entry point.
//!
//! * Target (`target_os = "none"`, llvm-i8008): `asm/entry.s` enters
//!   `monitor_main` from the cold (0x3B30) or warm (0x3B49) entry, or from a
//!   hold, which passes the hold number + 1 in B.
//! * Host: a teletype on stdin/stdout over a 16 KiB array, for trying the
//!   monitor by hand (`cargo run`). `G` prints where it would jump.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use core::panic::PanicInfo;
    use core::ptr::{read_volatile, write_volatile};
    use software::Hal;

    extern "C" {
        /// platform/runtime/tty.s
        fn __leaf_tty_getc() -> u8;
        fn __leaf_tty_putc(c: u8);
        /// asm/entry.s: load the saved registers and jump (does not return).
        fn mon_go(addr: u16);
        /// asm/entry.s: hold entries, the targets of the patched `CAL`s.
        fn mon_hold0();
        fn mon_hold1();
        /// asm/entry.s: the user's registers A B C D E H L and flags.
        static mut mon_regs: [u8; 8];
    }

    struct Board;
    impl Hal for Board {
        fn getc(&mut self) -> u8 {
            unsafe { __leaf_tty_getc() }
        }
        fn putc(&mut self, c: u8) {
            unsafe { __leaf_tty_putc(c) }
        }
        fn peek(&mut self, a: u16) -> u8 {
            unsafe { read_volatile(a as usize as *const u8) }
        }
        fn poke(&mut self, a: u16, v: u8) {
            unsafe { write_volatile(a as usize as *mut u8, v) }
        }
        fn reg(&mut self, i: u8) -> u8 {
            unsafe { read_volatile(&raw const mon_regs[i as usize]) }
        }
        fn set_reg(&mut self, i: u8, v: u8) {
            unsafe { write_volatile(&raw mut mon_regs[i as usize], v) }
        }
        fn hold_entry(&mut self, n: u8) -> u16 {
            (if n == 0 {
                mon_hold0 as usize
            } else {
                mon_hold1 as usize
            }) as u16
        }
        fn go(&mut self, a: u16) {
            unsafe { mon_go(a) }
        }
    }

    /// `hold` is 0 for the cold/warm entries, n + 1 after hold n.
    #[no_mangle]
    pub extern "C" fn monitor_main(hold: u8) -> ! {
        let mut b = Board;
        if hold != 0 {
            software::hold_reached(&mut b, hold - 1);
        }
        loop {
            software::command(&mut b);
        }
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

    struct Host {
        mem: Vec<u8>,
        regs: [u8; 8],
    }
    impl Hal for Host {
        fn getc(&mut self) -> u8 {
            let mut b = [0u8];
            if std::io::stdin().read(&mut b).unwrap_or(0) == 0 {
                std::process::exit(0);
            }
            let c = b[0] & 0x7f;
            let c = if c == b'\n' { b'\r' } else { c };
            c.to_ascii_uppercase()
        }
        fn putc(&mut self, c: u8) {
            let _ = std::io::stdout().write_all(&[c]);
            let _ = std::io::stdout().flush();
        }
        fn peek(&mut self, a: u16) -> u8 {
            self.mem[(a & 0x3fff) as usize]
        }
        fn poke(&mut self, a: u16, v: u8) {
            self.mem[(a & 0x3fff) as usize] = v;
        }
        fn reg(&mut self, i: u8) -> u8 {
            self.regs[i as usize]
        }
        fn set_reg(&mut self, i: u8, v: u8) {
            self.regs[i as usize] = v;
        }
        fn hold_entry(&mut self, n: u8) -> u16 {
            0x3800 + 8 * n as u16
        }
        fn go(&mut self, a: u16) {
            println!("(host: would jump to {a:04X})");
        }
    }

    let mut h = Host {
        mem: vec![0; 0x4000],
        regs: [0; 8],
    };
    loop {
        software::command(&mut h);
    }
}
