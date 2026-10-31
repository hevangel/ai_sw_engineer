//! Executor for the llvm-m68k qualification smoke image on this
//! repository's own MC68000 core (chip/motorola_68000, the core the
//! SingleStepTests 680x0 vector suite validates).
//!
//! Same contract as tools/r68k_run: loads the flat image at $00000400,
//! writes the reset vectors (SSP $00020000, PC $00000400) and the INPUT
//! stimulus, runs until the DONE sentinel at $1200 reads 0x5A, then
//! prints the results window (one byte per line) and the final PC to
//! stderr.
//!
//! Usage: m68000-run <smoke.bin> [window_bytes]
use std::path::PathBuf;

use motorola_68000::{Bus, Mc68000, Regs};

const IMAGE_BASE: u32 = 0x400;
const SSP: u32 = 0x0002_0000;
const INPUT_ADDR: u32 = 0x1000;
const DONE_ADDR: u32 = 0x1200;

const INPUT_BYTES: [u8; 8] = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];

/// Canonicalize the CLI-provided image path and refuse anything that
/// climbs out of the working directory (SAST path-traversal guard).
fn load_image(arg: &str) -> std::io::Result<Vec<u8>> {
    let mut p = PathBuf::from(arg).canonicalize()?;
    // Windows canonicalize returns a verbatim (\\?\) path; strip it so
    // the working-directory containment check compares like with like.
    if let Some(s) = p.to_str() {
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            p = PathBuf::from(stripped);
        }
    }
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

struct Mem {
    mem: Vec<u8>,
}

impl Mem {
    fn new() -> Self {
        Mem {
            mem: vec![0; 1 << 24],
        }
    }
}

impl Bus for Mem {
    fn read8(&mut self, addr: u32) -> u8 {
        self.mem[(addr & 0x00FF_FFFF) as usize]
    }
    fn write8(&mut self, addr: u32, val: u8) {
        self.mem[(addr & 0x00FF_FFFF) as usize] = val;
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let arg = args.next().expect(
        "usage: m68000-run <smoke.bin> [window_bytes] [start_pc] [input_addr] [output_addr] [done_addr]",
    );
    let window_bytes: u32 = args
        .next()
        .map_or(32, |s| s.parse().expect("window byte count"));
    let start_pc: u32 = args.next().map_or(IMAGE_BASE, |s| {
        u32::from_str_radix(s.trim_start_matches("0x"), 16)
            .expect("start pc hex address")
    });
    let input_addr: u32 = args.next().map_or(INPUT_ADDR, |s| {
        u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("input address")
    });
    let output_addr: u32 = args.next().map_or(0x1100, |s| {
        u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("output address")
    });
    let done_addr: u32 = args.next().map_or(DONE_ADDR, |s| {
        u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("done address")
    });
    let image = load_image(&arg).expect("read image");

    let mut mem = Mem::new();
    for (i, b) in image.iter().enumerate() {
        mem.mem[(IMAGE_BASE as usize + i) & 0x00FF_FFFF] = *b;
    }
    // Reset vector: longword SSP at $0, longword PC at $4.
    for (i, b) in SSP.to_be_bytes().iter().enumerate() {
        mem.mem[i] = *b;
    }
    for (i, b) in start_pc.to_be_bytes().iter().enumerate() {
        mem.mem[4 + i] = *b;
    }
    // Stimulus.
    for (i, b) in INPUT_BYTES.iter().enumerate() {
        mem.mem[(input_addr as usize + i) & 0x00FF_FFFF] = *b;
    }

    let mut cpu = Mc68000::new();
    cpu.set_state(Regs {
        d: [0; 8],
        a: [0; 7],
        usp: 0,
        ssp: SSP,
        pc: start_pc,
        sr: 0x2700, // supervisor, no trace, interrupt mask 7
    });

    let trace = std::env::var("M68000_RUN_TRACE").is_ok();
    if trace {
        let r = cpu.regs();
        eprintln!("[0000] pc={:06X} sp={:08X} sr={:04X}", r.pc, r.ssp, r.sr);
    }
    let cap: u64 = 200_000_000;
    let mut executed: u64 = 0;
    let mut exceptions = 0u32;
    loop {
        let info = cpu.step(&mut mem);
        executed += 1;
        if info.exception && exceptions < 8 {
            exceptions += 1;
            let r = cpu.regs();
            let opw = u16::from_be_bytes([
                mem.mem[(info.pc & 0xFF_FFFF) as usize],
                mem.mem[((info.pc + 1) & 0xFF_FFFF) as usize],
            ]);
            eprintln!(
                "exception #{} at instr pc={:06X} op={:04X} -> pc={:06X} sr={:04X} ssp={:08X}",
                exceptions, info.pc, opw, r.pc, r.sr, r.ssp
            );
        }
        if trace && executed < 400 {
            let r = cpu.regs();
            let opw = u16::from_be_bytes([
                mem.mem[(r.pc & 0xFF_FFFF) as usize],
                mem.mem[((r.pc + 1) & 0xFF_FFFF) as usize],
            ]);
            eprintln!(
                "[{:04}] pc={:06X} op={:04X} sp={:08X}",
                executed, r.pc, opw, r.ssp
            );
        }
        // Check every instruction: short programs (the mini is ~60)
        // finish long before a coarse polling window would fire.
        if mem.mem[done_addr as usize] == 0x5A || cpu.regs().pc == 0 {
            break;
        }
        if trace && executed % 1_000_000 == 0 {
            eprintln!("[{:012}] pc={:06X} sr={:04X}", executed, cpu.regs().pc, cpu.regs().sr);
        }
        if executed >= cap {
            let r = cpu.regs();
            panic!(
                "sentinel never set after {executed} instructions (pc={:06X} sr={:04X}) window={:02x?}",
                r.pc,
                r.sr,
                &mem.mem[output_addr as usize..output_addr as usize + 32]
            );
        }
    }

    for i in 0..window_bytes {
        println!("{:02x}", mem.mem[(output_addr + i) as usize]);
    }
    eprintln!("instructions {executed}");
}
