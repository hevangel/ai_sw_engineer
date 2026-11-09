//! The four-way matrix's Rust-emulator half: the original `PRINT.COM` and the Rust build, each
//! run by PC DOS 2.00 on this repository's Rust emulator (`system/ibm_pc/emulator`), on the same
//! cases as the oracle, typing the same keys after the same prompts. Compared with the oracle
//! (the original on 86Box's IBM PC/XT with a text printer on LPT1): the screen rows chosen by
//! the rule of `plans/testplan.md` (`common::compared`), and the bytes the printer took,
//! rendered as 86Box's text printer writes them (`common::render_text_printer`), against the
//! transcript's `#lpt|` lines.
//!
//! The Rust emulator models the IBM 5150 board (the XT's own Rust emulator is
//! ai_sw_engineer#29): the 5150 BIOS and Cassette BASIC from `$IBM_PC_ROMS`, 256 KiB of RAM
//! (`Machine::new(256)`), CGA, SW1 = 6Fh (two diskette drives), the IBM Printer Adapter at 378h
//! with an always-ready printer (`Machine::attach_printer`, emulator spec E-A13), PC DOS 2.00
//! disk 1 (`$XT_ROMS/pcdos200.img`, with the program under test as PRINT.COM and the case's
//! files added, as `../scripts/capture_dos.sh` builds it) in drive A, writable. After the last
//! prompt the machine runs, idle at `A>`, until the printer has taken nothing for
//! [`QUIET_SECONDS`] of machine time, so the background printing has finished.
//!
//! Run: `IBM_PC_ROMS=DIR XT_ROMS=DIR cargo test --release --test rust_emulator -- --ignored
//! --nocapture` (after `scripts/build.sh` for the Rust build).

mod common;

use std::path::{Path, PathBuf};
use system_emulator::machine::Machine;
use system_emulator::session::{self, Kind, Step};

/// The 8088's clock: 4.77 MHz.
const HZ: u64 = 4_772_727;
/// Machine time without a printed byte after which printing is taken as finished.
const QUIET_SECONDS: u64 = 15;
/// At most this much machine time after the last prompt.
const LIMIT_SECONDS: u64 = 300;

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

/// What a run shows: the rows compared (`common::compared`) and the printed lines.
struct Outcome {
    screen: Vec<String>,
    printed: Vec<String>,
    bytes: Vec<u8>,
}

/// Boots PC DOS 2.00 with `program` as PRINT.COM (None: the disk's own), runs the case, lets
/// the printing finish and gives what it showed and printed.
fn outcome(program: Option<&[u8]>, c: &common::Case, inputs: &Path) -> Outcome {
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
    let a = common::disk_a(&dos, program, &c.files, inputs);
    let mut m = Machine::new(256);
    m.attach_printer();
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, a.into_bytes(), false).unwrap();
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
    let limit = m.clock() + LIMIT_SECONDS * HZ;
    let (mut seen, mut since) = (0, m.clock());
    while m.clock() < limit && m.clock() - since < QUIET_SECONDS * HZ {
        m.run(100_000);
        let n = m.printed().unwrap().len();
        if n != seen {
            (seen, since) = (n, m.clock());
        }
    }
    let rows: Vec<String> = m
        .text_screen()
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect();
    let bytes = m.printed().unwrap().to_vec();
    Outcome {
        screen: common::compared(&common::transcript_rows(&c.name), &rows, &c.cmd).1,
        printed: common::render_text_printer(&bytes),
        bytes,
    }
}

/// Runs every case with `program`; the names of the cases that differ from the oracle.
fn matrix(program: Option<&[u8]>, label: &str) -> Vec<String> {
    let tmp = std::env::temp_dir().join(format!("print-emu-{}-{label}", std::process::id()));
    let mut bad = Vec::new();
    let cases = common::cases(&tmp);
    for case in &cases {
        let rows = common::transcript_rows(&case.name);
        let want = common::compared(&rows, &rows, &case.cmd).0;
        let want_printed = common::transcript_printed(&case.name);
        let got = outcome(program, case, &tmp);
        if got.screen == want && got.printed == want_printed {
            println!(
                "{label}, {}: identical to the oracle ({} bytes printed)",
                case.name,
                got.bytes.len()
            );
        } else {
            println!(
                "{label}, {}: `{}` differs\n  expected {want:?}\n  got      {:?}\n  \
                 expected printed {want_printed:?}\n  got      {:?}\n  bytes {:?}",
                case.name,
                case.cmd,
                got.screen,
                got.printed,
                String::from_utf8_lossy(&got.bytes)
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
    let com = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/print.com");
    std::fs::read(&com).expect("build/print.com: run scripts/build.sh")
}

#[test]
#[ignore = "needs IBM_PC_ROMS (5150 BIOS, BASIC) and XT_ROMS (pcdos200.img)"]
fn the_original_on_the_rust_emulator_matches_the_oracle() {
    assert!(matrix(None, "original PRINT.COM").is_empty());
}

#[test]
#[ignore = "needs the ROMs and build/print.com (scripts/build.sh)"]
fn the_rust_build_on_the_rust_emulator_matches_the_oracle() {
    assert!(matrix(Some(&rust_build()), "Rust PRINT.COM").is_empty());
}

#[test]
#[ignore = "needs build/print.com (scripts/build.sh)"]
fn the_build_has_the_layout_the_host_replay_models() {
    // the data ranges are the bytes the host's program image starts with, and the handlers are
    // where the host puts them (build/print.map)
    let com = rust_build();
    let image = common::resident_image();
    for (from, to) in common::IMAGE_DATA {
        for at in from..to {
            assert_eq!(
                com[usize::from(at) - 0x100],
                image[usize::from(at)],
                "byte {at:03X}h"
            );
        }
    }
    let map =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("build/print.map"))
            .unwrap();
    let h = common::HOOKS;
    for (name, at) in [
        ("print_int1c", h.int1c),
        ("print_int28", h.int28),
        ("print_int2f", h.int2f),
        ("print_int13", h.int13),
        ("print_int17", h.int17),
        ("print_int14", h.int14),
        ("print_int5", h.int5),
    ] {
        assert!(
            map.lines()
                .any(|l| l.trim() == format!("0x{at:04x} {name}")),
            "{name} at {at:03X}h"
        );
    }
    let sig = usize::from(h.int2f) - 0x100;
    assert_eq!(com[sig..sig + 13], common::SIGNATURE_BYTES);
}
