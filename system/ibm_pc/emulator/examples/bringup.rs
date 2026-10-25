//! Boot the original ROMs on the machine model and print the screen.
//!
//! Usage: `cargo run --release --example bringup -- [steps] [sw1_hex]`
//! `IBM_PC_DISK=<160 KiB image>` mounts a diskette in drive A (write protected).
//! ROMs come from `$IBM_PC_ROMS` (a directory with `bios.bin` and `basic.bin`),
//! else `$AI_HW_ENGINEER/system/ibm_pc/work/reference`. They are not in this
//! repository; see `original/provenance.md` for hashes and sources.

use std::path::PathBuf;
use system_emulator::cpu::Step;
use system_emulator::machine::Machine;

fn main() {
    let mut args = std::env::args().skip(1);
    let steps: u64 = args
        .next()
        .map(|s| s.parse().unwrap())
        .unwrap_or(20_000_000);
    let dir = std::env::var_os("IBM_PC_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let hw = std::env::var_os("AI_HW_ENGINEER")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("../../../../ai_hw_engineer"));
            hw.join("system/ibm_pc/work/reference")
        });
    let mut m = Machine::new(64);
    if let Some(sw1) = args.next() {
        m.board.sw1 = u8::from_str_radix(&sw1, 16).unwrap();
    }
    // BASIC (32 KiB) then the BIOS (8 KiB) on top of it.
    let mut rom = std::fs::read(dir.join("basic.bin")).unwrap();
    rom.extend(std::fs::read(dir.join("bios.bin")).expect("bios.bin"));
    m.load_rom_top(&rom).unwrap();
    if let Some(disk) = std::env::var_os("IBM_PC_DISK") {
        m.mount(0, std::fs::read(disk).expect("disk image"), true)
            .unwrap();
    }
    let mut last = 0;
    let mut shown = String::new();
    let watch = std::env::var_os("IBM_PC_SCREENLOG").is_some();
    for i in 0..steps {
        let s = m.step();
        if s != Step::Ran {
            println!(
                "stopped: {s:?} at {:04X}:{:04X} after {i} steps",
                m.cpu.s[1], m.cpu.ip
            );
            break;
        }
        if watch && i % 50_000 == 0 {
            let text = m
                .text_screen()
                .iter()
                .map(|l| l.trim_end().to_string())
                .collect::<Vec<_>>()
                .join(
                    "
",
                );
            if text != shown {
                println!(
                    "=== screen at step {i}, clock {}, {:04X}:{:04X}
{text}",
                    m.clock(),
                    m.cpu.s[1],
                    m.cpu.ip
                );
                shown = text;
            }
        }
        if i - last >= 1_000_000 {
            last = i;
            println!(
                "{i} steps, {} clocks, {:04X}:{:04X}",
                m.clock(),
                m.cpu.s[1],
                m.cpu.ip
            );
        }
    }
    if std::env::var_os("IBM_PC_IOLOG").is_some() {
        for (port, v) in &m.board.io_log {
            if matches!(*port, 0x3F2 | 0x3F5 | 0x00..=0x0F | 0x81) {
                println!("OUT {port:03X} {v:02X}");
            }
        }
    }
    if std::env::var_os("IBM_PC_DEBUG").is_some() {
        println!(
            "{:?}
{}",
            m.board.pic,
            m.board.fdc.summary()
        );
    }
    if std::env::var_os("IBM_PC_FDCLOG").is_some() {
        for (t, s) in &m.board.fdc.trace {
            println!("FDC {t:>11} {s}");
        }
    }
    println!(
        "--- {:04X}:{:04X} after {} clocks, {} instructions",
        m.cpu.s[1],
        m.cpu.ip,
        m.clock(),
        m.instructions
    );
    for line in m.text_screen() {
        println!("{}", line.trim_end());
    }
}
