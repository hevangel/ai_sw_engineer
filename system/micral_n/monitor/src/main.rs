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
        /// asm/entry.s: the hold slots.
        fn mon_patch(n: u8, addr: u16);
        fn mon_clear();
        static mon_holds: [u8; 10];
        /// asm/entry.s: the user's flags C Z S P and registers A B C D E H L.
        static mut mon_regs: [u8; 11];
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
        // `i` is at most REG_LAST (lib.rs `modify`), so no bounds check.
        fn reg(&mut self, i: u8) -> u8 {
            unsafe { read_volatile((&raw const mon_regs as *const u8).add(i as usize)) }
        }
        fn set_reg(&mut self, i: u8, v: u8) {
            unsafe { write_volatile((&raw mut mon_regs as *mut u8).add(i as usize), v) }
        }
        fn patch_hold(&mut self, n: u8, a: u16) {
            unsafe { mon_patch(n, a) }
        }
        fn clear_holds(&mut self) {
            unsafe { mon_clear() }
        }
        fn hold_addr(&mut self, n: u8) -> u16 {
            let i = if n == 0 { 0 } else { 5 };
            unsafe {
                u16::from_le_bytes([
                    read_volatile(&raw const mon_holds[i]),
                    read_volatile(&raw const mon_holds[i + 1]),
                ])
            }
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
        regs: [u8; 11],
        holds: [(u16, [u8; 3]); 2],
        nhold: u8,
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
        fn patch_hold(&mut self, n: u8, a: u16) {
            let n = n as usize;
            self.holds[n].0 = a;
            for k in 0..3 {
                self.holds[n].1[k] = self.mem[((a + k as u16) & 0x3fff) as usize];
            }
            let entry = 0x3800 + 8 * n as u16;
            for (k, v) in [0x46, entry as u8, (entry >> 8) as u8]
                .into_iter()
                .enumerate()
            {
                self.mem[((a + k as u16) & 0x3fff) as usize] = v;
            }
            self.nhold = n as u8 + 1;
        }
        fn clear_holds(&mut self) {
            for n in 0..self.nhold as usize {
                let (a, b) = self.holds[n];
                for (k, v) in b.into_iter().enumerate() {
                    self.mem[((a + k as u16) & 0x3fff) as usize] = v;
                }
            }
            self.nhold = 0;
        }
        fn hold_addr(&mut self, n: u8) -> u16 {
            self.holds[n as usize].0
        }
        fn go(&mut self, a: u16) {
            println!("(host: would jump to {a:04X})");
        }
    }

    let mut h = Host {
        mem: vec![0; 0x4000],
        regs: [0; 11],
        holds: [(0, [0; 3]); 2],
        nhold: 0,
    };
    // A `?` restarts the command loop by unwinding (lib.rs `cancel`).
    std::panic::set_hook(Box::new(|_| {}));
    loop {
        let _ =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| software::command(&mut h)));
    }
}
