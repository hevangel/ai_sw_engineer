//! Executor for the llvm-m68k qualification smoke image on the independent
//! r68k emulator (crates.io `r68k` 0.2.2, MIT — a Rust port of Musashi,
//! property-tested against Musashi; run, never copied).
//!
//! Loads the flat image at $00000400, writes the reset vectors (SSP
//! $00020000, PC $00000400) and the INPUT stimulus, runs until the DONE
//! sentinel at $1200 reads 0xA5, then prints the 32-byte results window
//! (one byte per line) and the final PC to stderr.
//!
//! Usage: r68k-run <smoke.bin>
use std::path::PathBuf;

use r68k::cpu::{ConfiguredCore, Core};
use r68k::interrupts::AutoInterruptController;
use r68k::ram::{AddressBus, PagedMem, SUPERVISOR_DATA};

const IMAGE_BASE: u32 = 0x400;
const SSP: u32 = 0x0002_0000;
const INPUT_ADDR: u32 = 0x1000;
const DONE_ADDR: u32 = 0x1200;

const INPUT_BYTES: [u8; 8] = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];

/// Canonicalize the CLI-provided image path and refuse anything that climbs
/// out of the working directory (SAST path-traversal guard).
fn load_image(arg: &str) -> std::io::Result<Vec<u8>> {
    let p = PathBuf::from(arg).canonicalize()?;
    let cwd = std::env::current_dir()?;
    if !p.starts_with(&cwd) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("image must live under the working directory: {arg}"),
        ));
    }
    let mut f = std::fs::File::open(p)?;
    let mut image = Vec::new();
    std::io::Read::read_to_end(&mut f, &mut image)?;
    Ok(image)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let arg = args.next().expect("usage: r68k-run <smoke.bin> [window_bytes]");
    let window_bytes: u32 = args.next().map_or(32, |s| s.parse().expect("window byte count"));
    let image = load_image(&arg).expect("read image");

    let mut mem = PagedMem::new(0);
    for (i, b) in image.iter().enumerate() {
        mem.write_byte(SUPERVISOR_DATA, IMAGE_BASE + i as u32, *b as u32);
    }
    // Reset vector: longword SSP at $0, longword PC at $4.
    mem.write_long(SUPERVISOR_DATA, 0x0000_0000, SSP);
    mem.write_long(SUPERVISOR_DATA, 0x0000_0004, IMAGE_BASE);
    // Stimulus.
    for (i, b) in INPUT_BYTES.iter().enumerate() {
        mem.write_byte(SUPERVISOR_DATA, INPUT_ADDR + i as u32, *b as u32);
    }

    let mut cpu = ConfiguredCore::new_with(0, AutoInterruptController::new(), mem);
    cpu.reset();

    let cap: u64 = 100_000_000;
    let mut executed: u64 = 0;
    loop {
        cpu.execute(10_000);
        executed += 10_000;
        if cpu.read_data_byte(DONE_ADDR).unwrap_or(0) == 0x5A {
            break;
        }
        if executed >= cap {
            panic!(
                "sentinel never set after {executed} cycles (pc={:06x})",
                cpu.pc()
            );
        }
    }

    for i in 0..window_bytes {
        println!("{:02x}", cpu.read_data_byte(0x1100 + i).unwrap_or(0));
    }
    eprintln!("cycles ~{executed} pc={:06x}", cpu.pc());
}
