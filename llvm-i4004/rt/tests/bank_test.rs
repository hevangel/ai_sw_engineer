// Minimal chips harness that models the xl ROM BANK LATCH (4001 chip #3 output
// port), to execution-test cross-bank calls before wiring the full oracle
// runner. The unmodified `external/chips` model has no bank concept, so this
// adapter:
//   * splits the image into a 1 KiB COMMON block (ROM chips #0..#3, always
//     bank 0) and N x 3 KiB BANKED blocks (ROM chips #4..#15);
//   * after each cycle reads the latch at b.roms[3].ports; on a change it
//     rebuilds b.roms[4..16] from the selected bank's 3 KiB (ROM::new is public,
//     b.roms is a public Vec) so the CPU fetches the new bank next.
//
// Usage: bank_test <image.bin> <cycles> <nchars>
//   image = common(1024) || bank0_hi(3072) || bank1_hi(3072) || ...
// Prints RAM chip0/reg0 chars 0..nchars (same readback as rt_test).
use chips::mcs4::Board;
use arbitrary_int::u4;

const COMMON: usize = 0x400; // chips #0..#3
const BANKED: usize = 0xC00; // chips #4..#15 (3 KiB)

fn make_image(common: &[u8], banked: &[u8], bank: usize) -> Vec<u8> {
    // Assemble the 4096-byte view the Board sees for `bank`.
    let mut v = vec![0u8; 0x1000];
    let c = common.len().min(COMMON);
    v[..c].copy_from_slice(&common[..c]);
    let off = bank * BANKED;
    if off < banked.len() {
        let n = (banked.len() - off).min(BANKED);
        v[COMMON..COMMON + n].copy_from_slice(&banked[off..off + n]);
    }
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let image = std::fs::read(&args[1]).unwrap();
    let cycles: usize = args[2].parse().unwrap();
    let nchars: usize = args[3].parse().unwrap();

    let common = &image[..COMMON.min(image.len())];
    let banked = if image.len() > COMMON { &image[COMMON..] } else { &[][..] };

    let mut bank = 0usize;
    let mut b = Board::new(make_image(common, banked, bank), 2);
    let mut prev_latch = b.roms[3].ports.value();

    for _ in 0..cycles {
        b.run_cycle();
        let latch = b.roms[3].ports.value() as usize;
        if latch != prev_latch as usize {
            prev_latch = latch as u8;
            if latch != bank {
                bank = latch;
                // Rebuild only the banked chips #4..#15 from the new bank; keep
                // the common chips #0..#3 and all RAM/CPU state intact.
                let view = make_image(common, banked, bank);
                for i in 4..16usize {
                    if i < b.roms.len() {
                        let start = i * 0x100;
                        let chunk: [u8; 0x100] =
                            view[start..start + 0x100].try_into().unwrap();
                        b.roms[i] = chips::mcs4::rom4001::ROM::new(chunk, u4::new(i as u8));
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for c in 0..nchars {
        out.push(b.rams[0].registers[0].characters.read_nibble(c as u8).value().to_string());
    }
    println!("{}", out.join(" "));
}
