//! run_fn — execute a raw 4004 ROM image on the real external/chips `Board` and
//! print observable RAM state, for EXECUTION tests of llvm-i4004-compiled code.
//!
//! This is a test harness, not a shipped component (same licence stance as
//! chips_xcheck: depends on external/chips by path, reads/runs it only).
//!
//! Usage:
//!   run_fn <rom.bin> <cycles>
//!
//! The ROM is a full driver program (built by scripts/exec_fn.py): it presets
//! argument registers, JMS-calls the compiled function, then copies the 8-bit
//! result (pair P1 = R2:R3) into RAM chip0 reg0 chars 0 (low nibble) and 1
//! (high nibble) with SRC+WRM, and parks in a NOP loop. After running `cycles`
//! instruction cycles we read those two nibbles back out of chips' public RAM
//! and print the reconstructed byte, plus the RAM output port (WMP) value.
//!
//! Output (stdout), one key=value per line:
//!   result=<0..255>     reconstructed R2:R3 (high<<4 | low) from reg0 char1,char0
//!   port=<0..15>        RAM output port (rams[0].ports), if the driver used WMP
//!   r2=<..> r3=<..>     raw nibbles (debug)

use chips::mcs4::Board;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: run_fn <rom.bin> <cycles>");
        std::process::exit(2);
    }
    let rom = fs::read(&args[1]).expect("read rom");
    let cycles: u32 = args[2].parse().expect("cycles");

    let mut image = rom;
    // Pad to at least 2 ROM chips (512 bytes); chips sizes ROM from the image
    // length. The second Board::new arg is the RAM chip count (match
    // chips_xcheck's convention of 2).
    if image.len() < 0x200 {
        image.resize(0x200, 0x00);
    }
    let mut board = Board::new(image, 2);

    for _ in 0..cycles {
        board.run_cycle();
    }

    // Readout region is RAM chip0 reg0: char0 = result low nibble, char1 = high.
    let lo = board.rams[0].registers[0].characters.read_nibble(0).value();
    let hi = board.rams[0].registers[0].characters.read_nibble(1).value();
    let c2 = board.rams[0].registers[0].characters.read_nibble(2).value();
    let c3 = board.rams[0].registers[0].characters.read_nibble(3).value();
    let port = board.rams[0].ports.value();
    let result = ((hi as u16) << 4 | lo as u16) & 0xFF;
    // 16-bit little-endian reconstruction for i16 tests: byte0 = (c1<<4|c0),
    // byte1 = (c3<<4|c2); value = byte1<<8 | byte0.
    let byte0 = ((hi as u16) << 4) | lo as u16;
    let byte1 = ((c3 as u16) << 4) | c2 as u16;
    let result16 = (byte1 << 8) | byte0;

    println!("result={}", result);
    println!("result16={}", result16);
    println!("port={}", port);
    println!("r3={}", lo);
    println!("r2={}", hi);
}
