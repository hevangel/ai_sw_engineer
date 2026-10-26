//! chkdsk entry point.
//!
//! * Bare-metal target (`target_os = "none"`, e.g. a `llvm-i8086` target):
//!   `no_std`/`no_main`; the platform runtime calls `main` after reset.
//! * Host: a small std program that drives the core from stdin/stdout, used by
//!   `scripts/run_host.sh` for interactive checks.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use core::panic::PanicInfo;

    /// Real-machine HAL. TODO: move into system/ibm_pc/platform/runtime/.
    struct MachineIo;

    impl software::Hal for MachineIo {
        fn read_input(&mut self) -> Option<u8> {
            None // TODO: read the machine's input device
        }
        fn write_output(&mut self, _value: u8) {
            // TODO: drive the machine's output device
        }
    }

    #[no_mangle]
    pub extern "C" fn main() -> ! {
        let mut machine = software::Machine::new();
        let mut io = MachineIo;
        loop {
            machine.step(&mut io);
        }
    }

    #[panic_handler]
    fn panic(_info: &PanicInfo) -> ! {
        loop {}
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    use std::io::{Read, Write};

    struct StdIo {
        input: std::vec::IntoIter<u8>,
        out: std::io::Stdout,
    }

    impl software::Hal for StdIo {
        fn read_input(&mut self) -> Option<u8> {
            self.input.next()
        }
        fn write_output(&mut self, value: u8) {
            let _ = self.out.write_all(&[value]);
        }
    }

    let mut buf = Vec::new();
    std::io::stdin()
        .read_to_end(&mut buf)
        .expect("failed to read stdin");
    let mut io = StdIo {
        input: buf.into_iter(),
        out: std::io::stdout(),
    };
    let mut machine = software::Machine::new();
    while machine.step(&mut io) {}
    let _ = io.out.flush();
}
