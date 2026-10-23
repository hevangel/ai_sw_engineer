//! run_ram — execute a 4004 ROM image on external/chips with CM-RAM bank 0
//! (four 4002s, 16 registers) preloaded, and dump bank-0 RAM afterwards.
//!
//! Used by the arithmetic-primitive execution tests (llvm-i4004/scripts/
//! exec_rt_num.py): the RAM image carries the operand registers in, the dump
//! carries the result registers and the primitive's return value out.
//!
//! Usage:
//!   run_ram <rom.bin> <ram-in-hex> <max-cycles>
//!
//! RAM image (in and out): 320 hex digits -- the 256 main characters in SRC
//! address order (`[chip:2][reg:2][char:4]`), then the 64 status characters,
//! register-major (`reg*4 + k`).
//!
//! The driver signals completion by writing 0xA to status character S3 of
//! register 0xF0; the run stops there (or after max-cycles).
//!
//! Output (stdout): `ram=<320 hex digits>` and `cycles=<n>` (and `done=0|1`).

use arbitrary_int::u4;
use chips::mcs4::Board;
use std::fs;

const DONE_REG: usize = 0xF; // register index 15 = SRC 0xF0
const DONE_MARK: u8 = 0xA;

/// `RUN_RAM_TRACE=1`: print chips' per-instruction debug lines (PC, ACC,
/// registers) to stderr, for debugging a routine under test.
struct Trace;
impl log::Log for Trace {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        eprintln!("{}", r.args());
    }
    fn flush(&self) {}
}

fn main() {
    if std::env::var_os("RUN_RAM_TRACE").is_some() {
        log::set_logger(&Trace).expect("logger");
        log::set_max_level(log::LevelFilter::Debug);
    }
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: run_ram <rom.bin> <ram-in-hex> <max-cycles>");
        std::process::exit(2);
    }
    let mut image = fs::read(&args[1]).expect("read rom");
    let ram_in: Vec<u8> = args[2]
        .chars()
        .map(|c| c.to_digit(16).expect("hex") as u8)
        .collect();
    assert_eq!(ram_in.len(), 320, "RAM image must be 320 nibbles");
    let max_cycles: u32 = args[3].parse().expect("cycles");
    // chips builds one 4001 per 256-byte chunk: pad to a whole page.
    let pages = (image.len().max(0x200) + 0xFF) / 0x100;
    if image.len() < pages * 0x100 {
        image.resize(pages * 0x100, 0x00);
    }
    let mut board = Board::new(image, 4);

    let reg_of = |r: usize| (r / 4, r % 4); // (chip, register)
    for a in 0..256usize {
        let (chip, reg) = reg_of(a >> 4);
        board.rams[chip].registers[reg]
            .characters
            .write_nibble((a & 15) as u8, u4::new(ram_in[a]));
    }
    for r in 0..16usize {
        let (chip, reg) = reg_of(r);
        for k in 0..4usize {
            board.rams[chip].registers[reg]
                .status
                .write_nibble(k as u8, u4::new(ram_in[256 + r * 4 + k]));
        }
    }
    let (dchip, dreg) = reg_of(DONE_REG);
    board.rams[dchip].registers[dreg].status.write_nibble(3, u4::new(0));

    let mut cycles = 0;
    let mut done = false;
    while cycles < max_cycles {
        board.run_cycle();
        cycles += 1;
        if cycles % 64 == 0
            && board.rams[dchip].registers[dreg].status.read_nibble(3).value() == DONE_MARK
        {
            done = true;
            break;
        }
    }

    let mut out = String::with_capacity(320);
    for a in 0..256usize {
        let (chip, reg) = reg_of(a >> 4);
        let v = board.rams[chip].registers[reg]
            .characters
            .read_nibble((a & 15) as u8)
            .value();
        out.push(char::from_digit(v as u32, 16).unwrap());
    }
    for r in 0..16usize {
        let (chip, reg) = reg_of(r);
        for k in 0..4usize {
            let v = board.rams[chip].registers[reg].status.read_nibble(k as u8).value();
            out.push(char::from_digit(v as u32, 16).unwrap());
        }
    }
    println!("ram={out}");
    println!("cycles={cycles}");
    println!("done={}", done as u8);
}
