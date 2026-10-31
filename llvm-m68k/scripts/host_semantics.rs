// Host differential check for the llvm-m68k smoke: the exact table_entry
// semantics, run on the host backend. Compile+run:
//   rustc -O --edition 2021 host_semantics.rs -o host_semantics && ./host_semantics
// The printed window must equal the Python oracle (expected.py) and the
// r68k execution dump. Whatever disagrees with host Rust is wrong.

fn input(i: usize) -> u8 {
    const INPUT_BYTES: [u8; 8] = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
    INPUT_BYTES[i]
}

fn table_entry(i: u8, a: u32, b: u32) -> u8 {
    let wide = (i as u32).wrapping_mul(0x0101_0101) ^ a.rotate_left((i % 7 + 1) as u32);
    let mixed = wide.wrapping_mul(251).wrapping_add(b & 0x00FF_00FF);
    let shifted = (mixed >> 3) | (mixed << (32 - 3));
    let counted = shifted.count_ones() as u32;
    let picked = if counted > (shifted & 0x3F) {
        counted
    } else {
        shifted & 0x3F
    };
    (picked ^ (mixed >> 24)) as u8
}

fn main() {
    let a = u32::from_be_bytes([input(0), input(1), input(2), input(3)]);
    let b = u32::from_be_bytes([input(4), input(5), input(6), input(7)]);
    let mut i: u8 = 0;
    loop {
        println!("{:02x}", table_entry(i, a, b));
        if i == 31 {
            break;
        }
        i += 1;
    }
    let confirm = (a.wrapping_mul(b) >> 24) as u8;
    println!("{:02x}", confirm);
    println!("{:02x}", 0xA5u8);
}
