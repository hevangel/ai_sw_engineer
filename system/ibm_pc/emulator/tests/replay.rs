//! Replay `oracle/transcripts/*.txt` (screens recorded from the hardware
//! project's RTL by `scripts/rtl_oracle.sh`) on the Rust machine and compare
//! the recorded screens and drive images exactly.
//!
//! The error-path tests need nothing. The replay needs the original IBM BIOS,
//! Cassette BASIC and PC DOS 1.00 disk, which are not in this repository: it
//! reads them from `$IBM_PC_ROMS` (a directory with `bios.bin`, `basic.bin`,
//! `pcdos100.img`, `workdisk.img`), else
//! `$AI_HW_ENGINEER/system/ibm_pc/work/reference`. SHA-256s are in
//! `original/provenance.md` and are checked first.
//!
//! Run: `cargo test --release --test replay -- --ignored --nocapture`

use std::path::{Path, PathBuf};
use system_emulator::machine::Machine;
use system_emulator::session::{self, Kind};
use system_emulator::sha256;

const BIOS_SHA256: &str = "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8";
const BASIC_SHA256: &str = "2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102";
const DOS_SHA256: &str = "29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2";

fn rom_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("IBM_PC_ROMS").map(PathBuf::from),
        std::env::var_os("AI_HW_ENGINEER")
            .map(|p| PathBuf::from(p).join("system/ibm_pc/work/reference")),
        Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../ai_hw_engineer/system/ibm_pc/work/reference"),
        ),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.join("bios.bin").exists())
}

fn read_checked(dir: &Path, name: &str, sha: &str) -> Vec<u8> {
    let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(
        sha256::hex(&bytes),
        sha,
        "{name} is not the original image in provenance.md"
    );
    bytes
}

#[test]
fn script_errors_are_reported_with_line_numbers() {
    for (text, want) in [
        ("until contains x\n", "line 1: until without boot or type"),
        ("boot\nuntil sideways x\n", "line 2: unknown condition"),
        ("boot\nuntil contains\n", "line 2: until needs"),
        ("boot\nnonsense\n", "line 2: unrecognized line"),
        ("type a\n", "boot or type without a following until"),
    ] {
        let err = session::parse(text).unwrap_err();
        assert!(err.contains(want), "{text:?}: {err}");
    }
    let steps = session::parse(
        "# c\nboot\nuntil row  Ok\nscreen\n|x\ndisk 1 sha256 00\ntype dir\\r\nuntil endswith A>\n",
    )
    .unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].kind, Kind::Row);
    assert_eq!(steps[0].needle, " Ok");
    assert_eq!(steps[1].keys.as_deref(), Some("dir\r"));
}

#[test]
fn firmware_sizes_are_checked() {
    let mut m = Machine::new(64);
    assert!(m.load_firmware(&[0; 100], None).is_err());
    assert!(m.load_firmware(&[0; 8192], Some(&[0; 100])).is_err());
    assert!(m.load_firmware(&[0; 8192], Some(&[0; 32768])).is_ok());
    assert!(m.mount(0, vec![0; 100], true).is_err());
    assert!(m.mount(4, vec![0; 163_840], true).is_err());
}

#[test]
#[ignore = "needs the original IBM BIOS, Cassette BASIC and PC DOS 1.00 disk (see module docs)"]
fn oracle_transcripts_replay_exactly() {
    let dir = rom_dir().expect("firmware not found: set IBM_PC_ROMS or AI_HW_ENGINEER");
    let bios = read_checked(&dir, "bios.bin", BIOS_SHA256);
    let basic = read_checked(&dir, "basic.bin", BASIC_SHA256);
    let dos = read_checked(&dir, "pcdos100.img", DOS_SHA256);
    let work = std::fs::read(dir.join("workdisk.img")).expect("workdisk.img");

    let transcripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle/transcripts");
    let mut files: Vec<_> = std::fs::read_dir(&transcripts)
        .expect("oracle/transcripts missing")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no transcripts in {}",
        transcripts.display()
    );

    for file in files {
        let expected = std::fs::read_to_string(&file).unwrap();
        let steps = session::parse(&expected).unwrap();
        let mut m = Machine::new(64);
        m.load_firmware(&bios, Some(&basic)).unwrap();
        m.mount(0, dos.clone(), true).unwrap();
        m.mount(1, work.clone(), false).unwrap();
        let got =
            session::run(&mut m, &steps).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        assert_eq!(
            session::strip_comments(&got),
            session::strip_comments(&expected),
            "{}: transcript differs from the RTL's",
            file.display()
        );
        println!(
            "{}: {} steps identical to the RTL",
            file.display(),
            steps.len()
        );
    }
}
