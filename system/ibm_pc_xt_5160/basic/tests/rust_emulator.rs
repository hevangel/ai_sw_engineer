//! The four-way matrix's Rust-emulator half: the original (BASIC.COM D2.00 on Cassette BASIC
//! C1.10) and the Rust build, each run by PC DOS 2.00 on this repository's Rust emulator
//! (`system/ibm_pc/emulator`), on the cases of `scripts/cases.py`, typing the same keys after
//! the same prompts, compared with the oracle (the original on MartyPC's IBM 5160) by the rule
//! of `plans/testplan.md` (`common::compared`).
//!
//! The Rust emulator models the IBM 5150 board: the 5150 BIOS from `$IBM_PC_ROMS`, the XT's
//! Cassette BASIC C1.10 (`$XT_ROMS/xtbasic.bin`, which BASIC.COM D2.00 needs) at F6000h, 256 KiB,
//! CGA, PC DOS 2.00 disk 1 (`$XT_ROMS/pcdos200.img`) with the program under test as BASIC.COM
//! and the case's files in drive A, the case's drive B disk in drive B.
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
        keys: keys.map(common::control_keys),
        kind,
        needle: needle.into(),
    }
}

/// A wait text: `$X` means the last non-blank row ends with X.
fn wait(text: &str) -> (Kind, &str) {
    match text.strip_prefix('$') {
        Some(t) => (Kind::EndsWith, t),
        None => (Kind::Contains, text),
    }
}

/// Boots PC DOS 2.00 with `program` as BASIC.COM (None: the disk's own), runs the case and
/// gives the whole screen at the end.
fn screen(program: Option<&[u8]>, c: &common::Case, inputs: &Path) -> Vec<String> {
    let roms = dir("IBM_PC_ROMS");
    let xt = dir("XT_ROMS");
    let bios = std::fs::read(roms.join("bios.bin")).unwrap();
    assert_eq!(
        sha(&bios),
        "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8"
    );
    let basic = std::fs::read(xt.join("xtbasic.bin")).unwrap();
    assert_eq!(
        sha(&basic),
        "3033d1a54c99d7e2aa1fc7c8c2e51a56ae1b61bf4e70e8aa580f43c43e37a63e"
    );
    let dos = std::fs::read(xt.join("pcdos200.img")).unwrap();
    assert_eq!(
        sha(&dos),
        "934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88"
    );
    let mut m = Machine::new(256);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, common::disk_a(&dos, program, &c.files, inputs), false)
        .unwrap();
    if let Some(b) = common::disk_b(&c.files, inputs) {
        m.mount(1, b, false).unwrap();
    }
    let mut steps = vec![
        step(None, Kind::Contains, "Enter new date"),
        step(Some("\r"), Kind::Contains, "Enter new time"),
        step(Some("\r"), Kind::EndsWith, "A>"),
    ];
    let mut keys = format!("{}\r", c.cmd);
    for (w, k) in &c.pairs {
        let (kind, needle) = wait(w);
        steps.push(step(Some(&keys), kind, needle));
        keys = k.clone();
    }
    if common::ends_at_dos(&common::transcript_rows(&c.name)) {
        steps.push(step(Some(&keys), Kind::EndsWith, "A>"));
    } else {
        steps.push(step(Some(&keys), Kind::Idle, "40000000"));
    }
    if let Err(e) = session::run(&mut m, &steps) {
        println!("{}: {e}", c.name);
    }
    m.text_screen()
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect()
}

/// Runs the cases (`only`: just those named) with `program`; the names of the cases that differ
/// from the oracle.
fn matrix(program: Option<&[u8]>, label: &str, only: Option<&[&str]>) -> Vec<String> {
    let tmp = std::env::temp_dir().join(format!("basic-emu-{}-{label}", std::process::id()));
    let mut bad = Vec::new();
    let cases: Vec<common::Case> = common::cases(&tmp)
        .into_iter()
        .filter(|c| only.is_none_or(|o| o.contains(&c.name.as_str())))
        .collect();
    for case in &cases {
        let oracle = common::transcript_rows(&case.name);
        let (want, _) = common::compared(&oracle, &oracle, &case.cmd);
        let got = common::compared(&oracle, &screen(program, case, &tmp), &case.cmd).1;
        if got == want {
            println!("{label}, {}: identical to the oracle", case.name);
        } else {
            println!(
                "{label}, {}: differs\n  expected {want:?}\n  got      {got:?}",
                case.name
            );
            bad.push(case.name.clone());
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
    println!(
        "{label} on the Rust emulator: {} of {} cases identical to the oracle",
        cases.len() - bad.len(),
        cases.len()
    );
    bad
}

fn rust_build() -> Vec<u8> {
    let com = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/basic.com");
    std::fs::read(&com).expect("build/basic.com: run scripts/build.sh")
}

#[test]
#[ignore = "needs IBM_PC_ROMS (5150 BIOS) and XT_ROMS (xtbasic.bin, pcdos200.img)"]
fn the_original_on_the_rust_emulator_matches_the_oracle() {
    assert!(matrix(None, "original BASIC.COM", None).is_empty());
}

#[test]
#[ignore = "needs the ROMs and build/basic.com (scripts/build.sh)"]
fn the_rust_build_on_the_rust_emulator_matches_the_oracle_on_the_milestone_cases() {
    // the cases of later milestones would wait for an `A>` that does not come (M2: `basic FILE`)
    let bad = matrix(Some(&rust_build()), "Rust BASIC.COM", Some(common::PASSING));
    assert!(bad.is_empty(), "milestone cases that differ: {bad:?}");
}
