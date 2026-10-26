//! Runner for the llvm-mos6502 qualification smoke image on the independent
//! mre/mos6502 emulator (external/mos6502, BSD-3-Clause; run, never copied).
//!
//! Loads the flat 32 KiB image at $8000, resets from the vector table, steps
//! until the DONE sentinel (address fixed by spec/link.ld + smoke layout) is
//! set, then prints the 32-byte results window and final registers. The
//! Python side (scripts/exec_mos6502.py) compares against the independent
//! expected values.
//!
//! Usage: mos6502-run <smoke.bin> [<done_addr_hex>] [<window_addr_hex>]

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;
use mos6502::memory::{Bus, Memory};
use std::fs;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: mos6502-run <smoke.bin> [done] [window]");
    let done_addr = args
        .next()
        .map_or(0x0220, |s| u16::from_str_radix(&s, 16).unwrap());
    let window_addr = args
        .next()
        .map_or(0x0200, |s| u16::from_str_radix(&s, 16).unwrap());

    let image = fs::read(&path).expect("read image");
    assert_eq!(image.len(), 32768, "expected the flat $8000..$FFFF image");

    let mut memory = Memory::new();
    memory.set_bytes(0x8000, &image);
    let mut cpu = CPU::new(memory, Nmos6502);
    cpu.reset();

    let cap: u64 = 50_000_000;
    let mut steps: u64 = 0;
    while cpu.memory.get_byte(done_addr) != 0x5A {
        cpu.single_step();
        steps += 1;
        if steps >= cap {
            panic!(
                "sentinel never set after {cap} instructions (pc={:04x})",
                cpu.registers.program_counter
            );
        }
    }

    for i in 0..32u16 {
        println!("{:02x}", cpu.memory.get_byte(window_addr + i));
    }
    eprintln!(
        "steps {steps} A={:02x} X={:02x} Y={:02x} SP={:02x} PC={:04x}",
        cpu.registers.accumulator,
        cpu.registers.index_x,
        cpu.registers.index_y,
        cpu.registers.stack_pointer.to_u16() & 0xFF,
        cpu.registers.program_counter
    );
}
