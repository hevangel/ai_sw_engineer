//! Log every printer hammer firing and paper advance while the last key of a
//! key list is processed (issue #23 debugging aid). Earlier keys run at full
//! speed through `advance`; the last key is single-stepped with the CPU probe,
//! holding the key for 64 half-spins as `advance` does.
//!
//!   cargo run --release --example hammer_trace -- <image> <dp> <rounding> <key>...
//!
//! Each event line: `<kind> pc=<pc> spin=<half-spin> phase=<cycle> drum=<char>`.
//! `drum` is the drum character index the printer decodes for a hammer fire.

use busicom_emulator::debug::Inputs;
use busicom_emulator::Emulator;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let image = std::fs::read(&a[0]).unwrap();
    let (dp, rnd): (u8, u8) = (a[1].parse().unwrap(), a[2].parse().unwrap());
    let keys: Vec<u8> = a[3..].iter().map(|k| k.parse().unwrap()).collect();
    let mut emu = Emulator::with_ram(&image, 16).unwrap();
    emu.advance(2, 0, 0, 2000).unwrap();
    for &k in &keys[..keys.len() - 1] {
        emu.advance(dp, rnd, k, 400).unwrap();
    }
    emu.enable_cpu_probe().unwrap();
    // HAMMER_TRACE_PC=lo-hi (hex) also prints every step with PC in that range.
    let window = std::env::var("HAMMER_TRACE_PC").ok().map(|s| {
        let (a, b) = s.split_once('-').unwrap();
        (
            u16::from_str_radix(a, 16).unwrap(),
            u16::from_str_radix(b, 16).unwrap(),
        )
    });
    let last = keys[keys.len() - 1];
    let mut prev = emu.ram_port(0).unwrap();
    let mut cycles = 0u64;
    let end = 400 * 1481u64;
    while cycles < end {
        let half_spins = cycles / 1481;
        let key = if half_spins < 64 { last } else { 0 };
        let step = emu
            .step_instruction(Inputs {
                precision: dp,
                rounding: rnd,
                key,
            })
            .unwrap();
        cycles += match (step.opcode >> 4, step.opcode & 1) {
            (1 | 4 | 5 | 7, _) | (2 | 3, 0) => 2,
            _ => 1,
        };
        let port = emu.ram_port(0).unwrap();
        let (spin, phase) = emu.drum();
        if let Some((lo, hi)) = window {
            if (lo..=hi).contains(&step.state.pc) {
                println!(
                    "  pc={:03X} op={:02X} test={} spin={spin} phase={phase} index={}",
                    step.state.pc,
                    step.opcode,
                    u8::from(step.state.test),
                    emu.rom_port(2).unwrap() & 1
                );
            }
        }
        if prev & 2 == 0 && port & 2 != 0 {
            println!(
                "hammer pc={:03X} spin={spin} phase={phase} drum={}",
                step.state.pc,
                (spin / 2 + 12) % 13
            );
        }
        if prev & 8 == 0 && port & 8 != 0 {
            println!("advance pc={:03X} spin={spin} phase={phase}", step.state.pc);
        }
        prev = port;
    }
}
