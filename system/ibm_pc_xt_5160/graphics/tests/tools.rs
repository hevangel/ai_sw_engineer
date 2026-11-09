//! The capture tools' own tests (tools/README.md), on the Rust emulator: PC DOS 2.00 booted as
//! in `rust_emulator.rs`, drive A with the tools (`build/tools`), drive B a blank 360 KiB disk.
//!
//! * DRAW leaves the CGA memory as tools/README.md documents it (`common::picture`): stopped
//!   at the BIOS's INT 5 entry (F000:FF54h), in modes 1, 3, 4, 5 and 6;
//! * PRNCAP keeps exactly what a printer takes: the BIOS Print Screen of DRAW's text picture
//!   sent to the emulator's own printer (`Machine::attach_printer`) and the same print kept by
//!   PRNCAP and written by PRNDUMP are the same bytes;
//! * the buffer: a second PRNCAP only says it is installed; PRNDUMP empties the buffer; three
//!   graphics prints (3 x 39,972 bytes) fill the 96 KiB buffer, PRNDUMP writes the first
//!   98,304 bytes (in pieces over 64 KiB) and counts the rest as lost;
//! * the messages: PRNDUMP without PRNCAP or without a name, DRAW with a bad mode.
//!
//! Run: `IBM_PC_ROMS=DIR XT_ROMS=DIR cargo test --release --test tools -- --ignored --nocapture`
//! (after `tools/build.sh`).

mod common;

use std::path::PathBuf;
use system_emulator::debug::{run_until, Stop};
use system_emulator::machine::Machine;
use system_emulator::session::{self, Kind, Step};

fn dir(var: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(var).unwrap_or_else(|| panic!("set {var}")))
}

/// A booted machine at `A>`: drive A PC DOS 2.00 disk 1 with the tools, drive B a blank 360 KiB
/// disk; `printer` fits the emulator's printer adapter.
fn booted(printer: bool) -> Machine {
    let roms = dir("IBM_PC_ROMS");
    let bios = std::fs::read(roms.join("bios.bin")).unwrap();
    let basic = std::fs::read(roms.join("basic.bin")).unwrap();
    let dos = std::fs::read(dir("XT_ROMS").join("pcdos200.img")).unwrap();
    let tools = common::tools_dir().expect("build/tools: run tools/build.sh");
    let files: Vec<String> = ["PRNCAP.COM", "PRNDUMP.COM", "DRAW.COM"]
        .map(String::from)
        .into();
    let mut m = Machine::new(256);
    if printer {
        m.attach_printer();
    }
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(
        0,
        common::disk_a(&dos, None, &files, &tools).into_bytes(),
        false,
    )
    .unwrap();
    let b = common::disk_b(&["b:label:PRINTOUT".into()]).unwrap();
    m.mount(1, b.into_bytes(), false).unwrap();
    let steps = [
        Step {
            keys: None,
            kind: Kind::Contains,
            needle: "Enter new date".into(),
        },
        Step {
            keys: Some("\r".into()),
            kind: Kind::Contains,
            needle: "Enter new time".into(),
        },
        Step {
            keys: Some("\r".into()),
            kind: Kind::EndsWith,
            needle: "A>".into(),
        },
    ];
    session::run(&mut m, &steps).unwrap();
    m
}

/// Types each command line and waits for the next `A>`.
fn commands(m: &mut Machine, lines: &[&str]) {
    let steps: Vec<Step> = lines
        .iter()
        .map(|l| Step {
            keys: Some(format!("{l}\r")),
            kind: Kind::EndsWith,
            needle: "A>".into(),
        })
        .collect();
    session::run(m, &steps).unwrap_or_else(|e| panic!("{lines:?}: {e}"));
}

fn rows(m: &Machine) -> Vec<String> {
    m.text_screen()
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect()
}

fn drive_b_file(m: &Machine, name: &str) -> Vec<u8> {
    let b = m.board.fdc.disks[1].as_ref().unwrap().data.clone();
    pcdisk::Disk::from_bytes(b).unwrap().read(name).unwrap()
}

#[test]
#[ignore = "needs IBM_PC_ROMS, XT_ROMS and build/tools"]
fn draw_leaves_the_documented_picture() {
    let font = common::font().expect("XT_ROMS: the ROM font");
    for mode in [1u8, 3, 4, 5, 6] {
        let mut m = booted(false);
        for code in system_emulator::keys::scan_codes(&format!("draw {mode}\r")) {
            m.key(code);
        }
        // the BIOS's INT 5 (no GRAPHICS installed)
        let stop = run_until(&mut m, 200_000_000, &[0xFFF54]);
        assert!(matches!(stop, Stop::Breakpoint(_)), "mode {mode}: {stop:?}");
        assert_eq!(m.read(0x449), mode);
        let want = common::picture(mode, Some(&font));
        let got = &m.vram()[..0x4000];
        let diff = (0..0x4000).find(|&i| got[i] != want[i]);
        assert_eq!(diff, None, "mode {mode}: the first byte that differs");
        println!("mode {mode}: the CGA memory is the documented picture");
    }
}

#[test]
#[ignore = "needs IBM_PC_ROMS, XT_ROMS and build/tools"]
fn prncap_keeps_what_a_printer_takes() {
    let mut m = booted(true);
    commands(&mut m, &["draw 3"]);
    let printer = m.printed().unwrap().to_vec();
    assert_eq!(printer.len(), 2 + 25 * 82);
    let mut m = booted(true);
    commands(&mut m, &["prncap", "draw 3", "prndump b:out.prn"]);
    assert!(
        m.printed().unwrap().is_empty(),
        "nothing reached the printer"
    );
    assert_eq!(drive_b_file(&m, "OUT.PRN"), printer);
    assert!(rows(&m).contains(&"2052 bytes".to_string()));
}

#[test]
#[ignore = "needs IBM_PC_ROMS, XT_ROMS, build/tools and the original GRAPHICS.COM on the disk"]
fn the_buffer_holds_96_kib_and_counts_the_rest() {
    let mut m = booted(false);
    commands(&mut m, &["prncap", "prncap"]);
    let r = rows(&m);
    assert!(r.contains(&"PRNCAP installed".to_string()), "{r:?}");
    assert!(r.contains(&"PRNCAP already installed".to_string()), "{r:?}");
    commands(&mut m, &["graphics", "draw 6", "prndump b:one.prn"]);
    let one = drive_b_file(&m, "ONE.PRN");
    assert_eq!(one.len(), 39972);
    // (PRNDUMP's usage message between the prints: each command line must change the screen)
    commands(
        &mut m,
        &[
            "draw 6",
            "prndump",
            "draw 6",
            "prndump",
            "draw 6",
            "prndump b:out.prn",
        ],
    );
    let r = rows(&m);
    assert!(r.contains(&"98304 bytes (21612 lost)".to_string()), "{r:?}");
    let three: Vec<u8> = one.iter().copied().cycle().take(3 * one.len()).collect();
    assert_eq!(drive_b_file(&m, "OUT.PRN"), three[..98304]);
    commands(&mut m, &["prndump b:empty.prn"]);
    assert!(drive_b_file(&m, "EMPTY.PRN").is_empty());
    assert!(rows(&m).contains(&"0 bytes".to_string()));
}

#[test]
#[ignore = "needs IBM_PC_ROMS, XT_ROMS and build/tools"]
fn the_messages() {
    let mut m = booted(false);
    commands(&mut m, &["prndump b:x.prn", "prndump", "draw 9", "draw"]);
    let r = rows(&m);
    for text in [
        "PRNCAP is not installed",
        "usage: PRNDUMP file",
        "usage: DRAW mode (0-6), then b for busy",
    ] {
        assert!(r.iter().any(|l| l == text), "{text}: {r:?}");
    }
}
