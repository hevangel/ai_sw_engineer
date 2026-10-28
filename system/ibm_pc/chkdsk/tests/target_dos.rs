//! The Rust `CHKDSK.COM` (`bin/chkdsk.com`, built by `scripts/build_com.sh` with
//! llvm-i8086) running under the unchanged PC DOS 1.00 on the Rust IBM PC
//! emulator, against the screens and disks the **original** `CHKDSK.COM` left on
//! the RTL (`oracle/transcripts`).
//!
//! The Rust program is added to the DOS disk as `RCHKDSK.COM`; every case boots
//! DOS, types the transcript's command with `chkdsk` replaced by `rchkdsk`, and
//! must show the same rows after the command and leave disk B with the recorded
//! hash. Needs the original BIOS, Cassette BASIC and PC DOS 1.00 disk, which are
//! not in the repository (see `tests/common/transcript.rs` for where they are
//! looked up); the test is ignored without `--ignored`.
//!
//! Run: `cargo test --release --test target_dos -- --ignored --nocapture`

mod common;

use common::transcript::{reference_dir, transcript, CASES};
use common::{disks, sha256};
use std::path::Path;
use system_emulator::machine::Machine;
use system_emulator::session;

const BIOS_SHA256: &str = "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8";
const BASIC_SHA256: &str = "2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102";
const DOS_SHA256: &str = "29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2";

fn read_checked(dir: &Path, name: &str, sha: &str) -> Vec<u8> {
    let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(sha256::hex(&bytes), sha, "{name} is not the original image");
    bytes
}

/// Boots DOS with `a` in drive A and `b` in drive B, types `command`, and returns the
/// screen rows and disk B afterwards.
fn run_under_dos(
    bios: &[u8],
    basic: &[u8],
    a: &[u8],
    b: Vec<u8>,
    command: &str,
) -> (Vec<String>, Vec<u8>) {
    // `\r` below is the two characters the session format uses for Enter
    let script = [
        "boot".to_string(),
        "until contains Enter today's date".to_string(),
        r"type 10-14-81\r".to_string(),
        "until endswith A>".to_string(),
        format!(r"type {command}\r"),
        "until endswith A>".to_string(),
    ]
    .join("\n");
    let steps = session::parse(&script).unwrap();
    let mut m = Machine::new(64);
    m.load_firmware(bios, Some(basic)).unwrap();
    m.mount(0, a.to_vec(), true).unwrap();
    m.mount(1, b, false).unwrap();
    session::run(&mut m, &steps).unwrap_or_else(|e| panic!("{command}: {e}"));
    let rows = m
        .text_screen()
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect();
    let disk_b = m.board.fdc.disks[1].as_ref().unwrap().data.clone();
    (rows, disk_b)
}

#[test]
fn the_com_file_is_the_one_the_build_script_makes() {
    let com = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("bin/chkdsk.com")).unwrap();
    let recorded = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("bin/chkdsk.com.sha256"),
    )
    .unwrap();
    assert_eq!(
        recorded.split_whitespace().next().unwrap(),
        sha256::hex(&com)
    );
    assert_eq!(com[0], 0x89, "starts with crt0_com");
    assert!(com.len() < 0xFE00);
}

#[test]
#[ignore = "needs the original IBM BIOS, Cassette BASIC and PC DOS 1.00 disk"]
fn the_rust_com_prints_what_the_original_printed_and_leaves_the_same_disk() {
    let dir = reference_dir().expect("firmware not found: set IBM_PC_ROMS or AI_HW_ENGINEER");
    let bios = read_checked(&dir, "bios.bin", BIOS_SHA256);
    let basic = read_checked(&dir, "basic.bin", BASIC_SHA256);
    let dos = read_checked(&dir, "pcdos100.img", DOS_SHA256);
    let com = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("bin/chkdsk.com")).unwrap();
    let mut a = pcdisk::Disk::from_bytes(dos.clone()).unwrap();
    a.add("RCHKDSK.COM", &com).unwrap();
    let a = a.into_bytes();

    let mut cases: Vec<(String, Vec<u8>)> = CASES
        .iter()
        .filter(|(name, _)| transcript(name).is_some())
        .map(|&(name, disk)| (name.to_string(), disks::build(disk)))
        .collect();
    cases.push((
        "001_workdisk".into(),
        std::fs::read(dir.join("workdisk.img")).unwrap(),
    ));
    cases.push(("003_dosdisk".into(), dos.clone()));
    cases.push(("011_no_args".into(), disks::build("d3_sizes")));

    for (name, b) in cases {
        let t = transcript(&name).unwrap();
        let mut typed = t.command.replacen("chkdsk", "rchkdsk", 1);
        // `chkdsk` alone checks drive A, so A must stay the original DOS disk: the
        // program is run from drive B instead.
        let (drive_a, drive_b) = if name == "011_no_args" {
            let mut b = pcdisk::Disk::from_bytes(b).unwrap();
            b.add("RCHKDSK.COM", &com).unwrap();
            typed = format!("b:{typed}");
            (dos.clone(), b.into_bytes())
        } else {
            (a.clone(), b)
        };
        let (rows, disk_b) = run_under_dos(&bios, &basic, &drive_a, drive_b, &typed);
        let screen = common::transcript::Transcript {
            command: typed.clone(),
            dos_sha: String::new(),
            before: String::new(),
            after: String::new(),
            rows,
        };
        assert_eq!(
            screen.rows_after(&typed),
            t.rows_after(&t.command),
            "{name}: screen differs from the original's on the RTL"
        );
        if name != "011_no_args" {
            assert_eq!(
                sha256::hex(&disk_b),
                t.after,
                "{name}: disk B differs from what the original left"
            );
        }
        println!("{name}: identical to the original on the RTL");
    }
}
