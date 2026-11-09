//! The pieces a front end needs, checked against the original firmware: the
//! speaker (the BIOS power-on beep), text drawn with the original CGA
//! character ROM, and the debugger stopping at the BIOS entry point.
//!
//! The expected values come from the original binaries and the RTL, not from
//! this emulator: the beep's timer divisor is read out of the BIOS image, and
//! the characters each text cell must show come from the RTL's recorded
//! screen (`oracle/transcripts/dos100_session.txt`), drawn with the glyphs in
//! the original character ROM.
//!
//! Needs `$IBM_PC_ROMS` with `bios.bin`, `basic.bin`, `pcdos100.img`,
//! `workdisk.img` and, for the text test, `cga.bin` (IBM CGA character ROM;
//! `original/provenance.md`). Run:
//! `cargo test --release --test frontend_support -- --ignored --nocapture`

use std::path::{Path, PathBuf};
use system_emulator::cga::font_8x8;
use system_emulator::debug::{self, Stop};
use system_emulator::machine::Machine;
use system_emulator::session;
use system_emulator::sha256;
use system_emulator::speaker::CPU_HZ;

const BIOS_SHA256: &str = "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8";
const BASIC_SHA256: &str = "2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102";
const DOS_SHA256: &str = "29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2";
const CGA_SHA256: &str = "37527f661580e5a09710051cec67422abfadf31c61b841537a91e7f27be19304";

fn rom_dir() -> PathBuf {
    [
        std::env::var_os("IBM_PC_ROMS").map(PathBuf::from),
        std::env::var_os("AI_HW_ENGINEER")
            .map(|p| PathBuf::from(p).join("system/ibm_pc/work/reference")),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.join("bios.bin").exists())
    .expect("firmware not found: set IBM_PC_ROMS")
}

fn read_checked(name: &str, sha: &str) -> Vec<u8> {
    let bytes = std::fs::read(rom_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(sha256::hex(&bytes), sha, "{name} is not the original image");
    bytes
}

fn machine() -> Machine {
    let mut m = Machine::new(64);
    m.load_firmware(
        &read_checked("bios.bin", BIOS_SHA256),
        Some(&read_checked("basic.bin", BASIC_SHA256)),
    )
    .unwrap();
    m
}

#[test]
#[ignore = "needs the original IBM BIOS and Cassette BASIC (see module docs)"]
fn the_power_on_beep_has_the_bios_divisor() {
    let bios = read_checked("bios.bin", BIOS_SHA256);
    // The BIOS BEEP routine (Technical Reference listing, BEEP) loads timer 2
    // with MOV AX,imm16 after MOV AL,0B6h / OUT 43h, writes both bytes to port
    // 42h, then turns the speaker on: IN AL,61h / MOV AH,AL / OR AL,3 / OUT 61h.
    // (The cassette test loads timer 2 the same way but then reads port 62h.)
    let head = [0xB0, 0xB6, 0xE6, 0x43, 0xB8];
    let tail = [
        0xE6, 0x42, 0x8A, 0xC4, 0xE6, 0x42, 0xE4, 0x61, 0x8A, 0xE0, 0x0C, 0x03, 0xE6, 0x61,
    ];
    let at = (0..bios.len() - 21)
        .find(|&i| bios[i..i + 5] == head && bios[i + 7..i + 21] == tail)
        .expect("BEEP's timer setup is in the BIOS");
    let divisor = u16::from_le_bytes([bios[at + 5], bios[at + 6]]);
    let expected_hz = 1_193_182.0 / f64::from(divisor);

    let mut m = machine();
    m.board.speaker.capture = true;
    // POST beeps once, after the memory test, before trying the diskette
    // (about 12 s of machine time on this model).
    let mut samples = Vec::new();
    let rate = 44_100;
    while m.clock() < 20 * CPU_HZ as u64 {
        m.run(20_000);
        let now = m.clock();
        m.board.speaker.render(now, rate, &mut samples);
    }
    assert!(m.board.speaker.transitions > 100, "the speaker moved");
    // Count rising crossings in the sound to measure its frequency.
    let first = samples.iter().position(|&s| s > 0.0).expect("some sound");
    let last = samples.iter().rposition(|&s| s > 0.0).unwrap();
    let rises = samples[first..=last]
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    let seconds = (last - first) as f64 / f64::from(rate);
    let hz = rises as f64 / seconds;
    println!(
        "divisor {divisor:#06x}: expected {expected_hz:.1} Hz, measured {hz:.1} Hz over {seconds:.3} s"
    );
    assert!(
        (hz - expected_hz).abs() / expected_hz < 0.02,
        "{hz} Hz vs {expected_hz} Hz"
    );
    assert!((0.1..2.0).contains(&seconds), "beep lasted {seconds} s");
}

#[test]
#[ignore = "needs the original IBM BIOS, PC DOS 1.00 and CGA character ROM (see module docs)"]
fn text_frames_use_the_character_rom_glyphs_of_the_rtl_screen() {
    let rom = read_checked("cga.bin", CGA_SHA256);
    let font = font_8x8(&rom).unwrap();
    let transcript = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle/transcripts/dos100_session.txt"),
    )
    .unwrap();
    // The first recorded step: boot to the date prompt.
    let steps = session::parse(&transcript).unwrap();
    let expected: Vec<String> = transcript
        .lines()
        .skip_while(|l| *l != "screen")
        .skip(1)
        .take(25)
        .map(|l| l.trim_start_matches('|').to_string())
        .collect();
    let mut m = machine();
    m.mount(0, read_checked("pcdos100.img", DOS_SHA256), true)
        .unwrap();
    session::run(&mut m, &steps[..1]).unwrap();

    // Cursor off so the cell under it is plain text.
    let (w, h, pixels) = {
        let mut cga = m.board.cga.clone();
        cga.write(0x3D4, 10);
        cga.write(0x3D5, 0x20);
        cga.frame(m.vram(), Some(font), 0).unwrap()
    };
    assert_eq!((w, h), (640, 200), "DOS runs in 80-column text");
    for (row, text) in expected.iter().enumerate() {
        let chars: Vec<u8> = format!("{text:<80}").bytes().collect();
        for (col, &ch) in chars.iter().enumerate() {
            for line in 0..8 {
                let glyph = font[usize::from(ch) * 8 + line];
                for x in 0..8 {
                    let lit = pixels[(row * 8 + line) * w + col * 8 + x] != 0;
                    assert_eq!(
                        lit,
                        glyph >> (7 - x) & 1 != 0,
                        "row {row} col {col} ({:?}) line {line} pixel {x}",
                        ch as char
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "needs the original IBM BIOS (see module docs)"]
fn the_debugger_stops_at_the_bios_entry_point() {
    let bios = read_checked("bios.bin", BIOS_SHA256);
    // The reset vector FFFF:0000 holds JMP FAR seg:off (EA lo hi lo hi).
    let v = &bios[0x1FF0..0x1FF5];
    assert_eq!(v[0], 0xEA, "reset vector is a far jump");
    let target = (u32::from(u16::from_le_bytes([v[3], v[4]])) << 4)
        + u32::from(u16::from_le_bytes([v[1], v[2]]));
    let mut m = machine();
    assert_eq!(
        debug::run_until(&mut m, 10, &[target]),
        Stop::Breakpoint(target)
    );
    assert_eq!(m.instructions, 1, "one far jump from the reset vector");
    let regs = debug::registers(&m);
    assert!(
        regs.contains(&format!("IP={:04X}", u16::from_le_bytes([v[1], v[2]]))),
        "{regs}"
    );
}

#[test]
#[ignore = "needs the original IBM BIOS and PC DOS 1.00 (see module docs)"]
fn copy_to_prn_reaches_the_printer_adapter() {
    // The BIOS finds the adapter at 378h at power-on (0040:0008) and INT 17h strobes each
    // byte; DOS's PRN device goes through INT 17h.
    let mut m = machine();
    m.attach_printer();
    let mut disk = pcdisk::Disk::from_bytes(read_checked("pcdos100.img", DOS_SHA256)).unwrap();
    disk.add("P.TXT", b"printed line\r\n\x1a").unwrap();
    m.mount(0, disk.into_bytes(), false).unwrap();
    let steps = session::parse(
        "boot\nuntil contains Enter today's date\ntype 1-01-80\\r\nuntil endswith A>\ntype copy p.txt prn\\r\nuntil endswith A>\n",
    )
    .unwrap();
    session::run(&mut m, &steps).unwrap();
    assert_eq!(m.printed(), Some(&b"printed line\r\n"[..]));
}
