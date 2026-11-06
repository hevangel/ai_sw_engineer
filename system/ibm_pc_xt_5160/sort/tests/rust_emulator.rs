//! The four-way matrix's Rust-emulator half: the original `SORT.EXE` and the Rust build, each run
//! by PC DOS 2.00 on this repository's Rust emulator (`system/ibm_pc/emulator`), on the same
//! cases as the oracle; the rows from the command line to the next prompt must equal the
//! oracle's (the original on MartyPC's IBM 5160).
//!
//! The Rust emulator models the IBM 5150 board (the XT's own Rust emulator is
//! ai_sw_engineer#29): the 5150 BIOS and Cassette BASIC from `$IBM_PC_ROMS`, 64 KiB, CGA, and PC
//! DOS 2.00 disk 1 (180 KiB, `$XT_ROMS/pcdos200.img`) in drive A, writable (pipes need it). SORT
//! and DOS see the same INT 21h interface either way.
//!
//! Run: `IBM_PC_ROMS=DIR XT_ROMS=DIR cargo test --release --test rust_emulator -- --ignored
//! --nocapture` (after `scripts/build.sh` for the Rust build).

use std::path::{Path, PathBuf};
use system_emulator::machine::Machine;
use system_emulator::session;

fn dir(var: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(var).unwrap_or_else(|| panic!("set {var}")))
}

fn sha(bytes: &[u8]) -> String {
    system_emulator::sha256::hex(bytes)
}

fn cases(out: &Path) -> Vec<(String, String, String)> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/cases.py");
    let o = std::process::Command::new("python3").arg("-I").arg(script).arg(out).output().unwrap();
    assert!(o.status.success());
    String::from_utf8(o.stdout)
        .unwrap()
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            (f[0].into(), f[1].into(), l[l.find("bytes").unwrap() + 5..].trim().into())
        })
        .collect()
}

/// Rows from the command line to the next `A>`.
fn command_rows(rows: &[String]) -> Vec<String> {
    let start = rows.iter().rposition(|r| r.starts_with("A>") && r.len() > 2).unwrap();
    let end = start + 1 + rows[start + 1..].iter().position(|r| r == "A>").unwrap();
    rows[start..=end].to_vec()
}

fn oracle(name: &str) -> Vec<String> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("oracle/transcripts/{name}.txt"));
    let rows: Vec<String> = std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l[1..].trim_end().to_string())
        .collect();
    command_rows(&rows)
}

/// Boots PC DOS 2.00 with `program` as SORT.EXE (None: the disk's own) and runs `cmd`.
fn run_case(program: Option<&[u8]>, file: &str, input: &[u8], cmd: &str) -> Vec<String> {
    let roms = dir("IBM_PC_ROMS");
    let xt = dir("XT_ROMS");
    let bios = std::fs::read(roms.join("bios.bin")).unwrap();
    let basic = std::fs::read(roms.join("basic.bin")).unwrap();
    assert_eq!(sha(&bios), "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8");
    let dos = std::fs::read(xt.join("pcdos200.img")).unwrap();
    assert_eq!(sha(&dos), "934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88");
    let mut disk = pcdisk::Disk::from_bytes(dos).unwrap();
    if let Some(p) = program {
        disk.add("SORT.EXE", p).unwrap();
    }
    disk.add(file, input).unwrap();
    let mut m = Machine::new(64);
    m.load_firmware(&bios, Some(&basic)).unwrap();
    m.mount(0, disk.into_bytes(), false).unwrap();
    let script = format!(
        "boot\nuntil contains Enter new date\ntype \\r\nuntil contains Enter new time\ntype \\r\n\
         until endswith A>\ntype {cmd}\\r\nuntil endswith A>\n"
    );
    let steps = session::parse(&script).unwrap();
    session::run(&mut m, &steps).unwrap_or_else(|e| panic!("{cmd}: {e}"));
    command_rows(&m.text_screen().iter().map(|r| r.trim_end().to_string()).collect::<Vec<_>>())
}

fn matrix(program: Option<Vec<u8>>, label: &str) {
    let tmp = std::env::temp_dir().join(format!("sort-emu-{}-{label}", std::process::id()));
    let mut n = 0;
    for (name, file, cmd) in cases(&tmp) {
        let input = std::fs::read(tmp.join(&file)).unwrap();
        let got = run_case(program.as_deref(), &file, &input, &cmd);
        // Both text dumps show bytes outside 20h-7Eh as spaces, except that MartyPC's keeps 7Fh;
        // 7Fh is compared as a space (a display convention, not SORT's output).
        let norm = |v: Vec<String>| -> Vec<String> { v.into_iter().map(|r| r.replace('\u{7f}', " ")).collect() };
        assert_eq!(norm(got), norm(oracle(&name)), "{label} on the Rust emulator, {name}: `{cmd}`");
        n += 1;
    }
    std::fs::remove_dir_all(&tmp).ok();
    println!("{label} on the Rust emulator: {n} cases identical to the oracle");
}

#[test]
#[ignore = "needs IBM_PC_ROMS (5150 BIOS, BASIC) and XT_ROMS (pcdos200.img)"]
fn the_original_on_the_rust_emulator_matches_the_oracle() {
    matrix(None, "original SORT.EXE");
}

#[test]
#[ignore = "needs the ROMs and build/sort.exe (scripts/build.sh)"]
fn the_rust_build_on_the_rust_emulator_matches_the_oracle() {
    let exe = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/sort.exe");
    matrix(Some(std::fs::read(&exe).expect("build/sort.exe: run scripts/build.sh")), "Rust SORT.EXE");
}
