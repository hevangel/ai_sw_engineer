// busicom_141pf HAL glue unit tests on the bare `chips` MCS-4 model.
//
// Runs a small ROM image that calls ONE HAL glue function, with the TEST pin
// driven to a chosen level, then reads back an observable: a RAM output port,
// a RAM main character (where the stub stored a returned value), or a ROM I/O
// port. The oracle is computed here in Rust, independent of the glue code.
//
// Usage:  hal_test <image.bin> <cycles> <test_level:0|1> <what> [arg]
//   what = ram_port <chip>     -> print b.rams[chip].ports
//          ram_char <chip> <c> -> print b.rams[chip].registers[0].characters[c]
//          rom_port <chip>     -> print b.roms[chip].ports
// The TEST pin is held at <test_level> for the whole run (set each cycle),
// matching how __bus_test_pin samples it. NOTE the chips TEST convention is
// inverted vs the board (platform/README.md): chips `set_test_flag(true)` makes
// JCN-C4 ("jump when TEST low") TAKE the branch, i.e. chips true == pin LOW as
// the manual/RTL see it. This harness passes the raw chips flag; the test
// oracle accounts for the inversion.
//
// The `chips` crate is a path dependency; run, never modified or copied.
use chips::mcs4::Board;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: hal_test <image.bin> <cycles> <test_level> <what> [arg..]");
        std::process::exit(2);
    }
    let image = std::fs::read(&a[1]).expect("image");
    let cycles: usize = a[2].parse().unwrap();
    let test_level: bool = a[3] != "0";
    let what = a[4].as_str();

    let mut b = Board::new(image, 2);
    for _ in 0..cycles {
        b.cpu.set_test_flag(test_level);
        b.run_cycle();
    }
    let out = match what {
        "ram_port" => {
            let chip: usize = a[5].parse().unwrap();
            b.rams[chip].ports.value() as u32
        }
        "ram_char" => {
            let chip: usize = a[5].parse().unwrap();
            let c: u8 = a[6].parse().unwrap();
            b.rams[chip].registers[0].characters.read_nibble(c).value() as u32
        }
        "rom_port" => {
            let chip: usize = a[5].parse().unwrap();
            b.roms[chip].ports.value() as u32
        }
        _ => {
            eprintln!("unknown what '{}'", what);
            std::process::exit(2);
        }
    };
    println!("{}", out);
}
