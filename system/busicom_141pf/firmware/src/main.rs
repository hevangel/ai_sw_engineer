//! Busicom 141-PF firmware entry point.
//!
//! * Target (`target_os = "none"`, `llvm-i4004`): the platform runtime calls
//!   `main` after reset. The keyboard and printer drivers are milestone M3.
//! * Host: reads key names from stdin (one per whitespace-separated token,
//!   same names as the oracle JSON; `dp=N` and `round=FL|54|IN` change the
//!   selectors) and prints each tape row.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod target {
    use core::panic::PanicInfo;
    use software::driver_keyboard::{KeyBuffer, KeyboardDriver};
    use software::driver_printer::PrinterDriver;
    use software::hal::Hal;
    use software::Calculator;

    // The concrete board `Hal` lowers each trait method to a single 4004 port
    // op. The port sequences themselves (SRC + WRR/RDR/WMP, JCN on TEST) live in
    // the platform runtime, exposed here as the four C-ABI `__bus_*` functions
    // below; the runtime's crt0 calls `main` after reset. The host still proves
    // the drivers against a real board model (`tests/port_level.rs`), so this
    // entry only binds the trait to those externs — no new behaviour.
    //
    // ASSUMPTION (M4): the runtime provides these symbols with exactly this
    // C ABI (recorded in llvm-i4004/rust/README.md and the brief). A method is
    // one SRC-select + one port op; only the low 4 bits of each nibble matter.
    extern "C" {
        /// `SRC chip` + `WRR`: write a ROM chip's 4-bit output port.
        fn __bus_rom_port_write(chip: u8, value: u8);
        /// `SRC chip` + `RDR`: read a ROM chip's 4-bit input port.
        fn __bus_rom_port_read(chip: u8) -> u8;
        /// `DCL`/`SRC chip` + `WMP`: write a RAM chip's 4-bit output port.
        fn __bus_ram_port_write(chip: u8, value: u8);
        /// `JCN`/`JTN` on the `TEST` pin: 1 = TEST HIGH (drum half-spin level).
        fn __bus_test_pin() -> u8;
        /// The lamp nibble (`Calculator::lamp_nibble`) to RAM1, read from the
        /// flags (asm/drivers.s).
        fn __bus_lamps();
    }

    /// The firmware main loop, generic over the board `Hal`: scan the keyboard,
    /// run a newly pressed key on the calculator driving the drum printer, and
    /// push the lamps out.
    #[inline(always)]
    pub fn run<H: Hal>(hal: &mut H) -> ! {
        let mut calc = Calculator::new();
        KeyBuffer::new(); // the scanner state: no key down
        loop {
            let rc = KeyboardDriver::new(hal).scan_key();
            if rc != 0xFF {
                let mut printer = PrinterDriver::new(hal);
                calc.press_at(rc >> 4, rc & 15, &mut printer);
                // `__bus_lamps` (asm/drivers.s): `calc.lamp_nibble()` to RAM1.
                unsafe { __bus_lamps() };
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn main() -> ! {
        let mut hal = BoardHal;
        run(&mut hal)
    }

    /// The board HAL: each method is one `__bus_*` call into the platform
    /// runtime (one 4004 port op). `test_pin` reads the TEST level (1 = HIGH).
    struct BoardHal;
    impl Hal for BoardHal {
        fn rom_port_write(&mut self, chip: u8, value: u8) {
            unsafe { __bus_rom_port_write(chip, value) }
        }
        fn rom_port_read(&mut self, chip: u8) -> u8 {
            unsafe { __bus_rom_port_read(chip) & 0xF }
        }
        fn ram_port_write(&mut self, chip: u8, value: u8) {
            unsafe { __bus_ram_port_write(chip, value) }
        }
        fn test_pin(&mut self) -> bool {
            unsafe { __bus_test_pin() != 0 }
        }
    }

    #[panic_handler]
    fn panic(_info: &PanicInfo) -> ! {
        loop {}
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    use software::{Calculator, Key, Printer, Rounding, Row, Switches};
    use std::io::Read;

    struct Tape;
    impl Printer for Tape {
        fn print(&mut self, row: &Row) {
            let cols: String = row.cols().iter().map(|&c| c as char).collect();
            println!(
                "{cols} {:<2}{:<2}{}",
                row.sym_a().glyph(),
                row.sym_b().glyph(),
                if row.red() { " (red)" } else { "" }
            );
        }
    }

    let names: &[(&str, u8)] = &[
        ("0", 156),
        ("1", 155),
        ("2", 151),
        ("3", 147),
        ("4", 154),
        ("5", 150),
        ("6", 146),
        ("7", 153),
        ("8", 149),
        ("9", 145),
        (".", 148),
        ("00", 152),
        ("+", 142),
        ("-", 141),
        ("*", 139),
        ("/", 138),
        ("=", 140),
        ("%", 134),
        ("ST", 137),
        ("EX", 158),
        ("CE", 159),
        ("C", 160),
        ("CM", 129),
        ("RM", 130),
        ("M-", 131),
        ("M+", 132),
        ("M=-", 135),
        ("M=+", 136),
        ("SIGN", 157),
        ("SQRT", 133),
    ];

    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .expect("failed to read stdin");
    let mut sw = Switches {
        dp: 2,
        rounding: Rounding::Truncate,
    };
    let mut calc = Calculator::new();
    for token in input.split_whitespace() {
        if let Some(v) = token.strip_prefix("dp=") {
            sw.dp = v.parse().expect("dp=N");
        } else if let Some(v) = token.strip_prefix("round=") {
            sw.rounding = match v {
                "FL" => Rounding::Float,
                "54" => Rounding::Round,
                "IN" => Rounding::Truncate,
                _ => panic!("round=FL|54|IN"),
            };
        } else {
            let code = names
                .iter()
                .find(|(n, _)| *n == token)
                .unwrap_or_else(|| panic!("unknown key {token:?}"))
                .1;
            calc.press(Key::from_code(code).unwrap(), sw, &mut Tape);
        }
    }
}
