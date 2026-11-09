//! The four-way matrix's Rust-emulator half: the original `GRAPHICS.COM` and the Rust build,
//! each run by PC DOS 2.00 on this repository's Rust emulator (`system/ibm_pc/emulator`), on the
//! same cases as the oracle, typing the same command lines after the same prompts. Each run is
//! compared with the oracle (the original on MartyPC's IBM 5160 with 256 KiB) twice: the 25
//! screen rows (`common::compared`), and the bytes PRNDUMP wrote to drive B's `OUT.PRN`, read
//! back from the emulator's diskette controller after the session (their length and SHA-256
//! against the transcript's `# printed:` line).
//!
//! The Rust emulator models the IBM 5150 board: the 5150 BIOS and Cassette BASIC from
//! `$IBM_PC_ROMS`, 256 KiB of RAM (`Machine::new(256)`), CGA, two diskette drives, PC DOS 2.00
//! disk 1 (`$XT_ROMS/pcdos200.img`) with the capture tools (`build/tools`, `tools/build.sh`) and
//! the program under test as GRAPHICS.COM in drive A, and the case's blank 360 KiB disk in
//! drive B, both writable, as `../scripts/capture_dos.sh` builds them. No printer is fitted:
//! PRNCAP.COM answers INT 17h.
//!
//! Run: `IBM_PC_ROMS=DIR XT_ROMS=DIR cargo test --release --test rust_emulator -- --ignored
//! --nocapture` (after `scripts/build.sh` for the Rust build). `CASE=<name>` runs one case.

mod common;

use std::path::{Path, PathBuf};
use system_emulator::machine::Machine;
use system_emulator::session::{self, Kind, Step};

fn dir(var: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(var).unwrap_or_else(|| panic!("set {var}")))
}

fn sha(bytes: &[u8]) -> String {
    system_emulator::sha256::hex(bytes)
}

fn step(keys: Option<&str>, kind: Kind, needle: &str) -> Step {
    Step {
        keys: keys.map(String::from),
        kind,
        needle: needle.into(),
    }
}

/// A wait text: `$X` means the last non-blank row ends with X (`until endswith`).
fn wait(text: &str) -> (Kind, &str) {
    match text.strip_prefix('$') {
        Some(t) => (Kind::EndsWith, t),
        None => (Kind::Contains, text),
    }
}

/// A booted machine with the case's disks: drive A PC DOS 2.00 disk 1 with `program` as
/// GRAPHICS.COM (None: the disk's own) and the tools, drive B the case's disk.
pub fn machine(program: Option<&[u8]>, c: &common::Case, inputs: &Path) -> Machine {
    let roms = dir("IBM_PC_ROMS");
    let xt = dir("XT_ROMS");
    let bios = std::fs::read(roms.join("bios.bin")).unwrap();
    let basic = std::fs::read(roms.join("basic.bin")).unwrap();
    assert_eq!(
        sha(&bios),
        "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8"
    );
    let dos = std::fs::read(xt.join("pcdos200.img")).unwrap();
    assert_eq!(
        sha(&dos),
        "934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88"
    );
    let mut m = Machine::new(256);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(
        0,
        common::disk_a(&dos, program, &c.files, inputs).into_bytes(),
        false,
    )
    .unwrap();
    if let Some(b) = common::disk_b(&c.files) {
        m.mount(1, b.into_bytes(), false).unwrap();
    }
    m
}

/// Runs the case: the screen rows and the bytes of drive B's OUT.PRN after it.
fn run_case(
    program: Option<&[u8]>,
    c: &common::Case,
    inputs: &Path,
) -> (Vec<String>, Option<Vec<u8>>) {
    let mut m = machine(program, c, inputs);
    let mut steps = vec![
        step(None, Kind::Contains, "Enter new date"),
        step(Some("\r"), Kind::Contains, "Enter new time"),
        step(Some("\r"), Kind::EndsWith, "A>"),
    ];
    // the command, then each key sequence once its prompt is on the screen
    let mut keys = format!("{}\r", c.cmd);
    for (w, k) in &c.pairs {
        let (kind, needle) = wait(w);
        steps.push(step(Some(&keys), kind, needle));
        keys = k.clone();
    }
    steps.push(step(Some(&keys), Kind::EndsWith, "A>"));
    session::run(&mut m, &steps).unwrap_or_else(|e| panic!("{}: {e}", c.cmd));
    let screen = m
        .text_screen()
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect();
    let b = m.board.fdc.disks[1].as_ref().map(|d| d.data.clone());
    let printed = b.and_then(|b| pcdisk::Disk::from_bytes(b).ok()?.read("OUT.PRN").ok());
    (screen, printed)
}

fn matrix(program: Option<Vec<u8>>, label: &str) {
    let tmp = std::env::temp_dir().join(format!("graphics-emu-{}-{label}", std::process::id()));
    common::tools_dir().expect("build/tools: run tools/build.sh");
    let only = std::env::var("CASE").ok();
    let (mut n, mut bad) = (0, Vec::new());
    for case in common::cases(&tmp) {
        if only.as_deref().is_some_and(|o| o != case.name) {
            continue;
        }
        n += 1;
        let (screen, printed) = run_case(program.as_deref(), &case, &tmp);
        let want = common::compared(&common::transcript_rows(&case.name));
        let got = common::compared(&screen);
        let (want_p, got_p) = (
            common::transcript_printed(&case.name),
            common::printed(printed.as_deref()),
        );
        if let (Some(dir), Some(p)) = (std::env::var_os("SAVE_DIR"), &printed) {
            std::fs::write(
                Path::new(&dir).join(format!("{label}_{}.prn", case.name)),
                p,
            )
            .unwrap();
        }
        if got == want && got_p == want_p {
            println!(
                "{label}, {}: identical to the oracle, printed {got_p:?}",
                case.name
            );
        } else {
            println!(
                "{label}, {}: differs\n  expected {want:?}\n  got      {got:?}\n  printed  {got_p:?}\n  expected {want_p:?}",
                case.name
            );
            bad.push(case.name.clone());
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
    println!(
        "{label} on the Rust emulator: {} of {n} cases identical to the oracle",
        n - bad.len()
    );
    assert!(bad.is_empty(), "{bad:?}");
}

fn rust_build() -> Vec<u8> {
    let com = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/graphics.com");
    std::fs::read(com).expect("build/graphics.com: run scripts/build.sh")
}

#[test]
#[ignore = "needs IBM_PC_ROMS (5150 BIOS, BASIC), XT_ROMS (pcdos200.img) and build/tools"]
fn the_original_on_the_rust_emulator_matches_the_oracle() {
    matrix(None, "original GRAPHICS.COM");
}

#[test]
#[ignore = "needs the ROMs, build/tools and build/graphics.com (scripts/build.sh)"]
fn the_rust_build_on_the_rust_emulator_matches_the_oracle() {
    matrix(Some(rust_build()), "Rust GRAPHICS.COM");
}

#[test]
#[ignore = "needs build/graphics.com (scripts/build.sh)"]
fn the_build_has_the_layout_of_the_spec() {
    // the handler, its data and the start at the original's offsets (build/graphics.map), the
    // jump at 100h and the short jump at 104h
    let map =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("build/graphics.map"))
            .unwrap();
    for (name, at) in [
        ("graphics_pixels", software::PIXELS),
        ("graphics_old5", software::OLD_INT5),
        ("graphics_int5", software::HANDLER),
        ("graphics_start", software::START),
    ] {
        assert!(
            map.lines()
                .any(|l| l.trim() == format!("0x{at:04x} {name}")),
            "{name} at {at:03X}h"
        );
    }
    let com = rust_build();
    assert_eq!(com[0..3], [0xE9, 0x32, 0x02]);
    assert_eq!(com[3..6], [0xFB, 0xEB, 0x0E]);
}
