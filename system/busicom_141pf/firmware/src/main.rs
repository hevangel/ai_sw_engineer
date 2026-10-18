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
    use software::driver_lamps::write_lamps;
    use software::driver_printer::PrinterDriver;
    use software::hal::Hal;
    use software::Calculator;

    // M4 supplies the concrete board `Hal` (SRC+WRR/RDR/WMP, JCN on TEST) via
    // `system/busicom_141pf/platform`. Until then the target entry uses a
    // placeholder HAL so the drivers link and `run` type-checks; the host
    // proves the drivers against a real board model (`tests/port_level.rs`).

    /// The firmware main loop, generic over the board `Hal`: scan the keyboard
    /// 40×/s into the 8-key buffer, run each buffered key on the calculator
    /// driving the drum printer, and push the lamps out.
    pub fn run<H: Hal>(hal: &mut H) -> ! {
        let mut calc = Calculator::new();
        let mut buffer = KeyBuffer::new();
        loop {
            let sw = {
                let mut kb = KeyboardDriver::new(hal);
                kb.scan_pass(&mut buffer)
            };
            while let Some(key) = buffer.pop() {
                {
                    let mut printer = PrinterDriver::new(hal);
                    calc.press(key, sw, &mut printer);
                }
                write_lamps(hal, calc.lamps());
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn main() -> ! {
        let mut hal = BoardHal;
        run(&mut hal)
    }

    /// Placeholder board HAL; replaced by the real `platform` implementation in
    /// M4. Present so `run` type-checks and the drivers link into the image.
    struct BoardHal;
    impl Hal for BoardHal {
        fn rom_port_write(&mut self, _chip: u8, _value: u8) {}
        fn rom_port_read(&mut self, _chip: u8) -> u8 {
            0
        }
        fn ram_port_write(&mut self, _chip: u8, _value: u8) {}
        fn test_pin(&mut self) -> bool {
            false
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
            let cols: String = row.cols.iter().map(|&c| c as char).collect();
            println!(
                "{cols} {:<2}{:<2}{}",
                row.sym_a.glyph(),
                row.sym_b.glyph(),
                if row.red { " (red)" } else { "" }
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
