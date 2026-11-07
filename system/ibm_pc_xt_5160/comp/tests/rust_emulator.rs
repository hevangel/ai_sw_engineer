//! The four-way matrix's Rust-emulator half: the original `COMP.COM` and the Rust build, each
//! run by PC DOS 2.00 on this repository's Rust emulator (`system/ibm_pc/emulator`), on the same
//! cases as the oracle, typing the same keys after the same prompts; the rows compared
//! (plans/testplan.md) must equal the oracle's (the original on MartyPC's IBM 5160).
//!
//! The Rust emulator models the IBM 5150 board (the XT's own Rust emulator is
//! ai_sw_engineer#29): the 5150 BIOS and Cassette BASIC from `$IBM_PC_ROMS`, 64 KiB, CGA, and PC
//! DOS 2.00 disk 1 (180 KiB, `$XT_ROMS/pcdos200.img`) in drive A, writable. COMP and DOS see the
//! same INT 21h interface either way.
//!
//! Run: `IBM_PC_ROMS=DIR XT_ROMS=DIR cargo test --release --test rust_emulator -- --ignored
//! --nocapture` (after `scripts/build.sh` for the Rust build).

use std::path::{Path, PathBuf};
use system_emulator::machine::Machine;
use system_emulator::session::{self, Kind, Step};

fn dir(var: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(var).unwrap_or_else(|| panic!("set {var}")))
}

fn sha(bytes: &[u8]) -> String {
    system_emulator::sha256::hex(bytes)
}

/// (name, files, command, (text to wait for, keys) pairs)
type Case = (String, Vec<String>, String, Vec<(String, String)>);

fn cases(out: &Path) -> Vec<Case> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/cases.py");
    let o = std::process::Command::new("python3")
        .arg("-I")
        .arg(script)
        .arg(out)
        .output()
        .unwrap();
    assert!(o.status.success());
    String::from_utf8(o.stdout)
        .unwrap()
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let files = f[1]
                .split(',')
                .filter(|s| *s != "-")
                .map(String::from)
                .collect();
            let pairs = f[3..]
                .chunks(2)
                .map(|p| (p[0].to_string(), p[1].to_string()))
                .collect();
            (f[0].into(), files, f[2].into(), pairs)
        })
        .collect()
}

fn screen_rows(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l[1..].trim_end().to_string())
        .collect()
}

/// The rows compared (plans/testplan.md): from the row after the command line to the next
/// `A>` when the oracle shows the command line; otherwise the oracle's rows from the top to its
/// last `A>`, against as many rows of the run ending at its last `A>`.
fn compared(rows: &[String], cmd: &str, visible: bool, n: usize) -> Vec<String> {
    let line = format!("A>{cmd}");
    if visible {
        let Some(c) = rows.iter().position(|r| *r == line) else {
            return vec![];
        };
        let Some(e) = rows[c + 1..].iter().position(|r| r == "A>") else {
            return vec![];
        };
        rows[c + 1..=c + 1 + e].to_vec()
    } else {
        let Some(end) = rows.iter().rposition(|r| r == "A>") else {
            return vec![];
        };
        rows[(end + 1).saturating_sub(n)..=end].to_vec()
    }
}

fn oracle(name: &str, cmd: &str) -> (Vec<String>, bool) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("oracle/transcripts/{name}.txt"));
    let rows = screen_rows(&std::fs::read_to_string(p).unwrap());
    let visible = rows.iter().any(|r| *r == format!("A>{cmd}"));
    (compared(&rows, cmd, visible, rows.len()), visible)
}

fn step(keys: Option<&str>, kind: Kind, needle: &str) -> Step {
    Step {
        keys: keys.map(String::from),
        kind,
        needle: needle.into(),
    }
}

/// Boots PC DOS 2.00 with `program` as COMP.COM (None: the disk's own) and runs the case.
fn run_case(program: Option<&[u8]>, tmp: &Path, c: &Case) -> Vec<String> {
    let (_, files, cmd, pairs) = c;
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
        disk.add("COMP.COM", p).unwrap();
    }
    for f in files {
        disk.add(f, &std::fs::read(tmp.join(f)).unwrap()).unwrap();
    }
    let mut m = Machine::new(64);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, disk.into_bytes(), false).unwrap();
    let mut steps = vec![
        step(None, Kind::Contains, "Enter new date"),
        step(Some("\r"), Kind::Contains, "Enter new time"),
        step(Some("\r"), Kind::EndsWith, "A>"),
    ];
    // the command, then each key sequence once its prompt is on the screen
    let mut keys = format!("{cmd}\r");
    for (wait, k) in pairs {
        steps.push(step(Some(&keys), Kind::Contains, wait));
        keys = k.clone();
    }
    steps.push(step(Some(&keys), Kind::EndsWith, "A>"));
    session::run(&mut m, &steps).unwrap_or_else(|e| panic!("{cmd}: {e}"));
    m.text_screen()
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect()
}

fn matrix(program: Option<Vec<u8>>, label: &str) {
    let tmp = std::env::temp_dir().join(format!("comp-emu-{}-{label}", std::process::id()));
    let mut n = 0;
    for c in cases(&tmp) {
        let (want, visible) = oracle(&c.0, &c.2);
        let screen = run_case(program.as_deref(), &tmp, &c);
        let got = compared(&screen, &c.2, visible, want.len());
        assert_eq!(
            got, want,
            "{label} on the Rust emulator, {}: `{}`",
            c.0, c.2
        );
        n += 1;
    }
    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(n, 10);
    println!("{label} on the Rust emulator: {n} cases identical to the oracle");
}

#[test]
#[ignore = "needs IBM_PC_ROMS (5150 BIOS, BASIC) and XT_ROMS (pcdos200.img)"]
fn the_original_on_the_rust_emulator_matches_the_oracle() {
    matrix(None, "original COMP.COM");
}

#[test]
#[ignore = "needs the ROMs and build/comp.com (scripts/build.sh)"]
fn the_rust_build_on_the_rust_emulator_matches_the_oracle() {
    let exe = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/comp.com");
    matrix(
        Some(std::fs::read(&exe).expect("build/comp.com: run scripts/build.sh")),
        "Rust COMP.COM",
    );
}
