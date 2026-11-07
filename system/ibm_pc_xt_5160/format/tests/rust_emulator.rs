//! The four-way matrix's Rust-emulator half: the original `FORMAT.COM` and the Rust build, each
//! run by PC DOS 2.00 on this repository's Rust emulator (`system/ibm_pc/emulator`), on the same
//! cases as the oracle, typing the same keys after the same prompts. Each run is compared with
//! the oracle (the original on MartyPC's IBM 5160 with 256 KiB) twice: the screen rows chosen
//! by the rule of `plans/testplan.md` (`common::compared`), and drive B's image read back from
//! the emulator's diskette controller (`common::drive_b_result`: its SHA-256, or the masked
//! one where the label's time or spec D1 makes the difference).
//!
//! The Rust emulator models the IBM 5150 board: the 5150 BIOS and Cassette BASIC from
//! `$IBM_PC_ROMS`, 256 KiB of RAM (`Machine::new(256)`), CGA, two diskette drives, PC DOS 2.00
//! disk 1 (180 KiB, `$XT_ROMS/pcdos200.img`) in drive A and the case's disk, built as
//! `../scripts/capture_dos.sh` builds it, in drive B, writable.
//!
//! Run: `IBM_PC_ROMS=DIR XT_ROMS=DIR cargo test --release --test rust_emulator -- --ignored
//! --nocapture` (after `scripts/build.sh` for the Rust build).
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

/// Boots PC DOS 2.00 with `program` as FORMAT.COM (None: the disk's own), `b` in drive B, and
/// runs the case; the screen rows and drive B's image after it.
fn run_case(
    program: Option<&[u8]>,
    b: Option<Vec<u8>>,
    c: &common::Case,
) -> (Vec<String>, Option<Vec<u8>>) {
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
    let mut disk = pcdisk::Disk::from_bytes(dos).unwrap();
    if let Some(p) = program {
        disk.add("FORMAT.COM", p).unwrap();
    }
    let mut m = Machine::new(256);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, disk.into_bytes(), false).unwrap();
    if let Some(b) = b {
        m.mount(1, b, false).unwrap();
    }
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
    (screen, b)
}

fn matrix(program: Option<Vec<u8>>, label: &str) {
    let tmp = std::env::temp_dir().join(format!("format-emu-{}-{label}", std::process::id()));
    let mut bad = Vec::new();
    let only = std::env::var("CASE").ok();
    let mut n = 0;
    for case in common::cases(&tmp) {
        if only.as_deref().is_some_and(|o| o != case.name) {
            continue;
        }
        n += 1;
        let b = common::drive_b(&case.setup, &tmp);
        let (screen, after) = run_case(program.as_deref(), b, &case);
        let (want, got) =
            common::compared(&common::transcript_rows(&case.name), &screen, &case.cmd);
        let disk = common::drive_b_result(&case.name, &case.cmd, after.as_deref());
        if let (Some(dir), Some(img)) = (std::env::var_os("SAVE_DIR"), &after) {
            let p = Path::new(&dir).join(format!("{label}_{}.img", case.name));
            std::fs::write(p, img).unwrap();
        }
        match (&disk, got == want) {
            (Ok(r), true) => println!("{label}, {}: screen identical, drive B {r}", case.name),
            _ => {
                println!(
                    "{label}, {}: `{}`\n  screen {}\n  drive B {disk:?}",
                    case.name,
                    case.cmd,
                    if got == want {
                        "identical".to_string()
                    } else {
                        format!("expected {want:?}\n  got      {got:?}")
                    }
                );
                bad.push(case.name.clone());
            }
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
    println!(
        "{label} on the Rust emulator: {} of {n} cases identical to the oracle",
        n - bad.len()
    );
    assert!(bad.is_empty(), "{label}: differs on {bad:?}");
    assert!(only.is_some() || n == 12);
}

#[test]
#[ignore = "needs IBM_PC_ROMS (5150 BIOS, BASIC) and XT_ROMS (pcdos200.img)"]
fn the_original_on_the_rust_emulator_matches_the_oracle() {
    matrix(None, "original FORMAT.COM");
}

#[test]
#[ignore = "needs the ROMs and build/format.com (scripts/build.sh)"]
fn the_rust_build_on_the_rust_emulator_matches_the_oracle() {
    let com = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/format.com");
    matrix(
        Some(std::fs::read(&com).expect("build/format.com: run scripts/build.sh")),
        "Rust FORMAT.COM",
    );
}
