//! Emulator side of the compiled-vs-host RAM comparison (issue #23 debugging
//! aid). Runs key codes on a firmware image (16 RAM chips) with the harness
//! timing of the manual-example schedule (2000 settle half-spins, then 400 per
//! key) and prints bank-0 RAM after each key. The host twin is
//! `firmware/examples/ram_trace.rs`; the output formats match.
//!
//!   cargo run --release --example ram_trace -- <image> <dp> <rounding> <key>...

use busicom_emulator::Emulator;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let image = std::fs::read(&a[0]).unwrap();
    let (dp, rnd): (u8, u8) = (a[1].parse().unwrap(), a[2].parse().unwrap());
    let mut emu = Emulator::with_ram(&image, 16).unwrap();
    for row in emu.advance(2, 0, 0, 2000).unwrap() {
        println!("setup {row}");
    }
    for k in &a[3..] {
        let key: u8 = k.parse().unwrap();
        for row in emu.advance(dp, rnd, key, 400).unwrap() {
            println!("row {row}");
        }
        let (mut main, mut status) = (String::new(), String::new());
        // Bank 0: chips 0..4, four registers each, in SRC order.
        for chip in 0..4 {
            for reg in 0..4u8 {
                for c in 0..16 {
                    main.push_str(&format!("{:X}", emu.ram_char(chip, reg, c).unwrap()));
                }
                for s in 0..4 {
                    status.push_str(&format!("{:X}", emu.ram_status(chip, reg, s).unwrap()));
                }
            }
        }
        for bank in 1..4usize {
            let mut line = String::new();
            for chip in bank * 4..bank * 4 + 4 {
                for reg in 0..4u8 {
                    for c in 0..16 {
                        line.push_str(&format!("{:X}", emu.ram_char(chip, reg, c).unwrap()));
                    }
                }
            }
            println!("bank {bank} {line}");
        }
        for bank in 1..4usize {
            let mut line = String::new();
            for chip in bank * 4..bank * 4 + 4 {
                for reg in 0..4u8 {
                    for c in 0..16 {
                        line.push_str(&format!("{:X}", emu.ram_char(chip, reg, c).unwrap()));
                    }
                }
            }
            println!("bank {bank} {line}");
        }
        println!("key {key}");
        println!("{main}");
        println!("{status}");
    }
}
