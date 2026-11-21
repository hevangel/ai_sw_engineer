//! Host side of the compiled-vs-host RAM comparison (issue #23 debugging aid).
//!
//! Runs a key list through the host core and prints bank-0 RAM after each key,
//! in the format of `emulator/examples/ram_trace.rs`, so the two can be diffed.
//!
//!   cargo run --example ram_trace -- <dp> <rounding-selector> <key-code>...
//!
//! Each line: `key <code>` then `<256 main nibbles> <64 status nibbles>` in hex.

use software::ram::host_dump;
use software::{Calculator, Key, Printer, Rounding, Row, Switches};

struct Sink;

impl Printer for Sink {
    fn print(&mut self, row: &Row) {
        let s: String = row.cols().iter().map(|&c| c as char).collect();
        println!("row {} {}", s.trim(), row.oracle_symbol());
    }
}

fn main() {
    let a: Vec<u8> = std::env::args().skip(1).map(|s| s.parse().unwrap()).collect();
    let sw = Switches {
        dp: a[0],
        rounding: Rounding::from_selector(a[1]).unwrap(),
    };
    let mut calc = Calculator::new();
    let mut sink = Sink;
    for &code in &a[2..] {
        calc.press(Key::from_code(code).unwrap(), sw, &mut sink);
        let img = host_dump();
        let hex: String = img.iter().map(|n| format!("{n:X}")).collect();
        println!("key {code}");
        println!("{}", &hex[..256]);
        println!("{}", &hex[256..]);
    }
}
