//! The four-way matrix's Rust-emulator half: the original `DISKCOMP.COM` and the Rust build,
//! each run by PC DOS 2.00 on this repository's Rust emulator (`system/ibm_pc/emulator`), on
//! the same cases as the oracle, typing the same keys after the same prompts. Compared with the
//! oracle (the original on MartyPC's IBM 5160 with 256 KiB): the rows chosen by the rule of
//! `plans/testplan.md` (`common::compared`). DISKCOMP writes nothing: each run also checks that
//! both drives are as they were mounted.
//!
//! The Rust emulator models the IBM 5150 board (the XT's own Rust emulator is
//! ai_sw_engineer#29): the 5150 BIOS and Cassette BASIC from `$IBM_PC_ROMS`, 256 KiB of RAM
//! (`Machine::new(256)`), CGA, SW1 = 6Fh (two double-sided diskette drives), PC DOS 2.00 disk 1
//! (`$XT_ROMS/pcdos200.img`, or its double-sided 360 KiB copy for `a:system360`, with the
//! program under test as DISKCOMP.COM) in drive A and the case's 360 KiB disk (for `b:copy_a` a
//! copy of that drive A), built as `../scripts/capture_dos.sh` builds them, in drive B, both
//! writable.
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

/// Boots PC DOS 2.00 with `program` as DISKCOMP.COM (None: the disk's own) and the case's
/// disks, runs the case, and gives the rows compared (`common::compared`).
fn outcome(program: Option<&[u8]>, c: &common::Case, inputs: &Path) -> Vec<String> {
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
    let (a, b) = common::disks(&dos, program, &c.setup, inputs);
    let mut m = Machine::new(256);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, a.clone(), false).unwrap();
    if let Some(b) = &b {
        m.mount(1, b.clone(), false).unwrap();
    }
    let mut steps = vec![
        step(None, Kind::Contains, "Enter new date"),
        step(Some("\r"), Kind::Contains, "Enter new time"),
        step(Some("\r"), Kind::EndsWith, "A>"),
    ];
    // the command, then each key once its prompt is on the screen
    let mut keys = format!("{}\r", c.cmd);
    for (w, k) in &c.pairs {
        let (kind, needle) = wait(w);
        steps.push(step(Some(&keys), kind, needle));
        keys = k.clone();
    }
    steps.push(step(Some(&keys), Kind::EndsWith, "A>"));
    session::run(&mut m, &steps).unwrap_or_else(|e| panic!("{}: {e}", c.cmd));
    let screen: Vec<String> = m
        .text_screen()
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect();
    let disk = |i: usize| m.board.fdc.disks[i].as_ref().map(|d| d.data.clone());
    assert_eq!(disk(0).as_ref(), Some(&a), "{}: drive A written", c.name);
    assert_eq!(disk(1), b, "{}: drive B written", c.name);
    common::compared(&common::transcript_rows(&c.name), &screen, &c.cmd).1
}

fn oracle(c: &common::Case) -> Vec<String> {
    let rows = common::transcript_rows(&c.name);
    common::compared(&rows, &rows, &c.cmd).0
}

/// Runs every case with `program`; the names of the cases that differ from the oracle.
fn matrix(program: Option<&[u8]>, label: &str) -> Vec<String> {
    let tmp = std::env::temp_dir().join(format!("diskcomp-emu-{}-{label}", std::process::id()));
    let mut bad = Vec::new();
    let cases = common::cases(&tmp);
    for case in &cases {
        let want = oracle(case);
        let got = outcome(program, case, &tmp);
        if got == want {
            println!("{label}, {}: identical to the oracle", case.name);
        } else {
            println!(
                "{label}, {}: `{}` differs\n  expected {want:?}\n  got      {got:?}",
                case.name, case.cmd
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
    let com = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/diskcomp.com");
    std::fs::read(&com).expect("build/diskcomp.com: run scripts/build.sh")
}

#[test]
#[ignore = "needs IBM_PC_ROMS (5150 BIOS, BASIC) and XT_ROMS (pcdos200.img)"]
fn the_original_on_the_rust_emulator_matches_the_oracle() {
    assert!(matrix(None, "original DISKCOMP.COM").is_empty());
}

#[test]
#[ignore = "needs the ROMs and build/diskcomp.com (scripts/build.sh)"]
fn the_rust_build_on_the_rust_emulator_matches_the_oracle() {
    assert!(matrix(Some(&rust_build()), "Rust DISKCOMP.COM").is_empty());
}

#[test]
#[ignore = "needs the ROMs and build/diskcomp.com (scripts/build.sh)"]
fn the_rust_build_does_what_the_original_does_on_the_rust_emulator() {
    let tmp = std::env::temp_dir().join(format!("diskcomp-emu-{}-both", std::process::id()));
    let com = rust_build();
    let mut n = 0;
    for case in common::cases(&tmp) {
        let original = outcome(None, &case, &tmp);
        let rewrite = outcome(Some(&com), &case, &tmp);
        assert_eq!(rewrite, original, "{}", case.name);
        n += 1;
    }
    std::fs::remove_dir_all(&tmp).ok();
    println!("the Rust build gives the original's screen on {n} of {n} cases");
}
