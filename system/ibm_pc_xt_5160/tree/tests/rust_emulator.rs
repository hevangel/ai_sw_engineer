//! The four-way matrix's Rust-emulator half: the original `TREE.COM` and the Rust build, each
//! run by PC DOS 2.00 on this repository's Rust emulator (`system/ibm_pc/emulator`), on the same
//! cases as the oracle; the rows chosen by the rule of `plans/testplan.md` (`common::compared`)
//! must equal the oracle's (the original on MartyPC's IBM 5160).
//!
//! The Rust emulator models the IBM 5150 board (the XT's own Rust emulator is
//! ai_sw_engineer#29): the 5150 BIOS and Cassette BASIC from `$IBM_PC_ROMS`, 64 KiB, CGA, SW1 =
//! 6Fh (two diskette drives), PC DOS 2.00 disk 1 (180 KiB, `$XT_ROMS/pcdos200.img`) in drive A
//! and the case's 360 KiB disk, built with pcdisk as `../scripts/capture_dos.sh` builds it, in
//! drive B. TREE and DOS see the same INT 21h interface either way.
//!
//! Run: `IBM_PC_ROMS=DIR XT_ROMS=DIR cargo test --release --test rust_emulator -- --ignored
//! --nocapture` (after `scripts/build.sh` for the Rust build).

mod common;

use std::path::{Path, PathBuf};
use system_emulator::machine::Machine;
use system_emulator::session;

fn dir(var: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(var).unwrap_or_else(|| panic!("set {var}")))
}

fn sha(bytes: &[u8]) -> String {
    system_emulator::sha256::hex(bytes)
}

/// Boots PC DOS 2.00 with `program` as TREE.COM (None: the disk's own), `b` in drive B, and
/// runs `cmd`; the screen rows.
fn run_case(program: Option<&[u8]>, b: Option<pcdisk::Disk>, cmd: &str) -> Vec<String> {
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
        disk.add("TREE.COM", p).unwrap();
    }
    let mut m = Machine::new(64);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, disk.into_bytes(), false).unwrap();
    if let Some(b) = b {
        m.mount(1, b.into_bytes(), false).unwrap();
    }
    let script = format!(
        "boot\nuntil contains Enter new date\ntype \\r\nuntil contains Enter new time\ntype \\r\n\
         until endswith A>\ntype {cmd}\\r\nuntil endswith A>\n"
    );
    let steps = session::parse(&script).unwrap();
    session::run(&mut m, &steps).unwrap_or_else(|e| panic!("{cmd}: {e}"));
    m.text_screen()
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect()
}

fn matrix(program: Option<Vec<u8>>, label: &str) {
    let tmp = std::env::temp_dir().join(format!("tree-emu-{}-{label}", std::process::id()));
    let mut n = 0;
    for case in common::cases(&tmp) {
        let b = common::drive_b(&case.setup, &tmp);
        let screen = run_case(program.as_deref(), b, &case.cmd);
        let (want, got) = common::compared(&common::transcript_rows(&case.name), &screen);
        assert_eq!(
            got, want,
            "{label} on the Rust emulator, {}: `{}`",
            case.name, case.cmd
        );
        n += 1;
    }
    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(n, 9);
    println!("{label} on the Rust emulator: {n} cases identical to the oracle");
}

#[test]
#[ignore = "needs IBM_PC_ROMS (5150 BIOS, BASIC) and XT_ROMS (pcdos200.img)"]
fn the_original_on_the_rust_emulator_matches_the_oracle() {
    matrix(None, "original TREE.COM");
}

#[test]
#[ignore = "needs the ROMs and build/tree.com (scripts/build.sh)"]
fn the_rust_build_on_the_rust_emulator_matches_the_oracle() {
    let com = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/tree.com");
    matrix(
        Some(std::fs::read(&com).expect("build/tree.com: run scripts/build.sh")),
        "Rust TREE.COM",
    );
}
