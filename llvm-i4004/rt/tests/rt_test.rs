// libi4004rt unit-test harness on the V. Ilmer `chips` MCS-4 model.
//
// Runs a small ROM image on a bare 4004 + 2x4002 (no board I/O, no drum/printer)
// for a fixed number of machine cycles, then reads back result nibbles from
// 4002 chip 0 and prints them. The image is a generated test program
// (tools/gen_rt_test.py) that loads inputs into argument pairs, JMS'es one
// runtime helper, writes the 8-bit result (and, for div, the remainder) to RAM
// main characters, then spins in a self-JUN halt loop. The host harness
// compares the read-back nibbles against a reference computed in Rust, so the
// oracle is an INDEPENDENT host computation, never the helper's own output.
//
// Usage:  rt_test <image.bin> <cycles> <nchars>
//   prints `nchars` nibble values (RAM chip0, reg0, chars 0..nchars-1), space
//   separated, after running `cycles` machine cycles. Exit 0 always; the
//   caller (gen_rt_test.py / run_rt_tests.sh) does the comparison.
//
// The `chips` crate is a path dependency on the pinned external checkout; it is
// run, never modified or copied (no license).
use chips::mcs4::Board;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: rt_test <image.bin> <cycles> <nchars>");
        std::process::exit(2);
    }
    let image = std::fs::read(&args[1]).expect("read image");
    let cycles: usize = args[2].parse().expect("cycles");
    let nchars: usize = args[3].parse().expect("nchars");

    let mut b = Board::new(image, 2);
    for _ in 0..cycles {
        b.run_cycle();
    }
    // Read RAM chip 0, register 0, characters 0..nchars.
    let mut out = Vec::new();
    for c in 0..nchars {
        let v = b.rams[0].registers[0].characters.read_nibble(c as u8);
        out.push(v.value().to_string());
    }
    println!("{}", out.join(" "));
}
