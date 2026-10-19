// busicom_141pf secondary-oracle trace adapter.
//
// Runs ANY 4004 ROM image (1280..4096 bytes) on the unmodified V. Ilmer `chips`
// MCS-4 board model and prints the port-level tape in EXACTLY the
// oracle/rom_port_trace.txt format ("#EXAMPLE/#RESET/#STEP n", "LAMPS|n",
// "ROW|value|symA|symB|red").
//
// It generalises ai_hw_engineer/scripts/reference_oracle.rs: the only change is
// that the ROM image path is argv[1] (any image, not just the original mask),
// so the SAME board wiring and drum decode that produced the committed oracle
// is reused to score our own compiled images. The `chips` crate itself is never
// modified or copied; it is a path dependency on the pinned external checkout.
//
// Board wiring / drum tables: documented facts in platform/io_map.md and the
// ai_hw_engineer board spec; mirrors external/busicom's keyboard/printer protocol.
//
// TEST-pin note: `chips` jumps on JCN-C4 when its internal `test` flag is HIGH
// (cpu4004.rs: `c4 & self.test`), the INVERSE of the MCS-4 manual / the
// ai_hw_engineer RTL (which jump when the TEST pin is LOW). This adapter drives
// `set_test_flag(spin%2==0)`, the same value reference_oracle.rs uses, so the
// compensation is identical to the oracle capture and the two agree. See
// platform/README.md "JCN TEST-pin polarity".
use chips::{mcs4::{Board, shifter4003::Shifter}, shifter::Direction};
use arbitrary_int::u4;

// Banked (xl) ROM support. A banked image from i4004_pack --banks is laid out as
//   common(COMMON_BYTES) || bank0_tail(BANK_BYTES) || bank1_tail(BANK_BYTES) ...
// where COMMON_BYTES of common ROM is mapped in EVERY bank (chips 0..COMMON/256)
// and BANK_BYTES is the per-bank window (chips COMMON/256..16). The firmware
// sets a bank LATCH on 4001 chip 3's output port (SRC 0x30, WRR) via the
// __tobank_* stubs and __soft_ret; this adapter watches roms[3].ports and swaps
// the banked chips' content on a change (the chips model itself has no bank
// concept, and external/ is read-only). A flat image (<= 4096 B) uses no banks.
const COMMON_BYTES_DEFAULT: usize = 0x800;
const BANK_BYTES_DEFAULT: usize = 0x800;

// Build the 4096-byte ROM view the Board sees for `bank`: common region then
// the selected bank's tail.
fn bank_view(common: &[u8], banked: &[u8], bank: usize, bank_bytes: usize) -> Vec<u8> {
    let mut v = vec![0u8; 0x1000];
    let c = common.len().min(0x1000);
    v[..c].copy_from_slice(&common[..c]);
    let off = bank * bank_bytes;
    if off < banked.len() {
        let n = (banked.len() - off).min(bank_bytes).min(0x1000 - common.len());
        v[common.len()..common.len() + n].copy_from_slice(&banked[off..off + n]);
    }
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let image = std::fs::read(&args[1]).unwrap();
    let schedule = std::fs::read_to_string(&args[2]).unwrap();
    // Banked image? (produced by i4004_pack --banks; size > 4096.)
    let common_bytes = std::env::var("COMMON_BYTES").ok()
        .and_then(|s| usize::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(COMMON_BYTES_DEFAULT);
    let bank_bytes = std::env::var("BANK_BYTES").ok()
        .and_then(|s| usize::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(BANK_BYTES_DEFAULT);
    let banked_image = image.len() > 0x1000;
    let common: Vec<u8> = if banked_image { image[..common_bytes].to_vec() } else { Vec::new() };
    let banked: Vec<u8> = if banked_image { image[common_bytes..].to_vec() } else { Vec::new() };
    let first_banked_chip = common_bytes / 0x100;
    // initial board image: bank 0 view for a banked image, else the flat image.
    let make_initial = |img: &[u8]| -> Vec<u8> {
        if banked_image { bank_view(&common, &banked, 0, bank_bytes) } else { img.to_vec() }
    };
    // RAM 4002 count: stock board = 2 (bank 0 chips 0,1). The xl board passes
    // a larger count as argv[3] (or $CHIPS_RAM_COUNT) to model the added 4002s
    // across the four CM-RAM banks (chips grouped in banks of 4). rams[0]/[1]
    // keep the printer/lamp roles regardless, so the tape decode is unchanged.
    let ram_count: u8 = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("CHIPS_RAM_COUNT").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(2);
    let mut b = Board::new(make_initial(&image), ram_count);
    let mut cur_bank = 0usize;
    let mut prev_latch = 0u8;  // 8-bit latch (port4<<4|port3); bank 0 at reset
    let mut shifts = [Shifter::new(), Shifter::new(), Shifter::new()];
    let mut spin = 0usize;
    let mut previous = 0;
    let mut red = false;
    let mut row = vec![" "; 18];
    let digits = ["0","1","2","3","4","5","6","7","8","9",".",".","-"];
    let sa = ["◇","+","-","×","÷","M+","M-","^","=","√","%","C","R"];
    let sb = ["#","*","Ⅰ","Ⅱ","Ⅲ","M+","M-","T","K","E","Ex","C","M"];
    for line in schedule.lines() {
        if line.starts_with("#EXAMPLE") {
            b = Board::new(make_initial(&image), ram_count);
            cur_bank = 0; prev_latch = 0;
            shifts = [Shifter::new(), Shifter::new(), Shifter::new()];
            spin = 0; previous = 0; red = false; row.fill(" ");
        }
        if line.starts_with('#') {
            println!("{}", line);
            if line.starts_with("#STEP") { println!("LAMPS|{}", b.rams[1].ports.value()); }
            continue;
        }
        let v: Vec<usize> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
        let (precision, rounding, key, duration) = (v[0], v[1], v[2], v[3]);
        for tick in 0..duration {
            let current = if tick < 64 { key } else { 0 };
            for _ in 0..1481 {
                b.run_cycle();
                // xl bank latch (8-bit): low nibble on 4001 chip-3's output
                // port, high nibble on chip-4's. __soft_ret writes the HIGH
                // nibble (chip 4) FIRST, then the LOW nibble (chip 3), so the
                // chip-3 write is the COMMIT: trigger the banked-ROM reload only
                // on a chip-3 change and read the (already-settled) high nibble
                // from chip 4. Triggering on either nibble would reload at the
                // transient (newhi<<4)|oldlo mid-update and map a wrong bank
                // while control is still in the dispatch. bank=(port4<<4)|port3;
                // 8 bits lifts the 16-bank ceiling to the 45+ banks the firmware
                // needs.
                if banked_image {
                    let lo = b.roms[3].ports.value();
                    if lo != prev_latch {
                        prev_latch = lo;
                        let bankn = (((b.roms[4].ports.value() as usize) & 0xF) << 4)
                            | ((lo as usize) & 0xF);
                        if bankn != cur_bank {
                            cur_bank = bankn;
                            let view = bank_view(&common, &banked, cur_bank, bank_bytes);
                            for i in first_banked_chip..16usize {
                                if i < b.roms.len() {
                                    let s = i * 0x100;
                                    let chunk: [u8; 0x100] =
                                        view[s..s + 0x100].try_into().unwrap();
                                    b.roms[i] = chips::mcs4::rom4001::ROM::new(
                                        chunk, u4::new(i as u8));
                                }
                            }
                        }
                    }
                }
                let p = b.roms[0].ports.value();
                shifts[0].read_write_serial(Direction::Left, p & 2 == 0, p & 1 == 0);
                let out = shifts[1].read_write_serial(Direction::Left, p & 2 == 2, p & 4 == 0);
                shifts[2].read_write_serial(Direction::Left, out, p & 4 == 0);
                let scan = shifts[0].read_parallel() as usize;
                let keys = if current >= 129 && scan == 1 << ((current - 129) / 4) {
                    1 << ((current - 129) % 4)
                } else { 0 };
                let input = if scan == 256 { precision } else if scan == 512 { rounding } else { keys };
                b.roms[1].ports = u4::new(input as u8);
                let p = b.rams[0].ports.value();
                if previous & 8 == 0 && p & 8 != 0 {
                    if row.iter().any(|s| *s != " ") {
                        println!("ROW|{}|{}|{}|{}", row[..15].join("").trim(), row[16], row[17], red);
                    }
                    row.fill(" "); red = false;
                }
                if previous & 2 == 0 && p & 2 != 0 {
                    let pos = (spin / 2 + 12) % 13;
                    let bits = shifts[1].read_parallel() as u32 | ((shifts[2].read_parallel() as u32) << 10);
                    for i in 0..15 { if bits & (1 << (i + 3)) != 0 { row[i] = digits[pos]; } }
                    if bits & 1 != 0 { row[16] = sa[pos]; }
                    if bits & 2 != 0 { row[17] = sb[pos]; }
                }
                if p & 1 != 0 { red = true; }
                previous = p;
            }
            b.cpu.set_test_flag(spin % 2 == 0);
            if spin % 2 == 0 {
                let mut p = b.roms[2].ports.value();
                if spin == 26 { p |= 1; spin = 0; } else { p &= 14; }
                b.roms[2].ports = u4::new(p);
            }
            spin += 1;
        }
    }
}
